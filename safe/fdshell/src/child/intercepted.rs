//! The intercepted-builtin arm of a pipeline stage's dispatch (task #381): the
//! twin of `child/user_function.rs`. A stage is a full command, so its name
//! resolves the plain path's order — a user function first, then **both**
//! builtin tables, then `PATH`. `run/parent.rs` is the plain path's arm; this is
//! the arm the *stage* fork needs, and it runs after everything `child_main`
//! owns (the capture socket, the stage's redirects, arg substitution, the scoped
//! prefix and the trace).
//!
//! The whole intercepted table answers here, not a subset of it: the child
//! already owns **its own copy** of the state (`sys::fork_pidfd_cell`), so every
//! mutation a handler makes — cwd, options, positionals, exports, aliases,
//! frames — dies with the fork, and that is bash's stage-subshell rule (bash
//! runs every builtin in the stage's own subshell, and a stage's `cd` cannot
//! move the parent's base either). One rule, one gate: the five parse-keyword
//! names (`break`, `continue`, `return`, `unset`, `umask`) have no builtins at
//! all, so they keep their `not found` answer.

use crate::error::child_process::ChildProcessError;
use crate::parse::CommandLine;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ScriptText, ShortCStr};

/// The arm's gate, asked by `child_main` so the trace goes to the arm that runs
/// it: the intercepted table answers this stage name, and the arms ahead of it —
/// the forked table (`builtin`) and a user function (`fn_body`) — did not.
/// `stage` is task #481's gate: the plain launch resolves intercepts in the
/// parent, so it never arrives here.
pub(crate) fn answers(
    name: &ShortCStr,
    builtin: bool,
    stage: bool,
    fn_body: &Option<ShortCStr>,
) -> bool {
    let raw = name.as_bytes().unwrap_or(&[]);
    stage && !builtin && fn_body.is_none() && crate::intercept::commands::is_intercepted(raw)
}

/// Run the stage's intercepted builtin in this child and report the code the
/// child exits with: its own last status, the `pipeline/child/block.rs` rule. A
/// loop control the handler raises is the same `Ok` arm — it dies with the child,
/// only the status is reported. A report of the handler's own is the caller's
/// report arm: `pipeline::launch_pipeline` prints it and exits with its code, so
/// the visible rc stays the **pipeline's** (the last stage's), as bash's is.
/// `Ok(None)` — "not intercepted" — is unreachable from the arm: the gate already
/// tested the name.
pub(super) fn run(
    text: &ScriptText,
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<ChildProcessError>> {
    // No cell borrow is held across the handler: `eval` and `source` re-enter
    // the interpreter, which borrows the same cell (LESSONS: never hold a
    // ForkCell borrow across a recursive call that re-borrows). The handler gets
    // the **raw** `cmdline` — it runs its own substitution — so the `resolved`
    // list `child_main` built for the exec path is not used here.
    crate::intercept::try_intercept(text, cmdline, cell)
        .map(|_| last_status_code(cell))
        .change_context(ChildProcessError::StageFailed)
}

/// The child's own last status, the code `sys::exit` carries
/// (`async_job::last_status_code` is the model). An un-borrowable state is an
/// invariant breach with no child-side report channel: status 1.
fn last_status_code(cell: &ForkCell<ShellState>) -> i32 {
    cell.borrow()
        .map(|state| state.last_status.exit_code())
        .unwrap_or(1)
}
