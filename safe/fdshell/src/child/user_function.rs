//! The user-function arm of a pipeline stage's dispatch (task #481): a stage is
//! a full command, so its name resolves the way a plain command's does — a user
//! function first, then the builtin table, then `PATH`. The plain path's own
//! function arm is `run/parent.rs`; this one is the arm the *stage* fork needs,
//! and it runs after everything `child_main` owns (the capture socket, the
//! stage's redirects, arg substitution, the scoped prefix and the trace).

use crate::error::child_process::ChildProcessError;
use crate::parse::CommandLine;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ScriptText, ShortCStr};

/// Run the stage's function call in this child and report the code the child
/// exits with: the function's own last status, the `pipeline/child/block.rs`
/// rule (a stage runs in a subshell, so the function's `cd`, assignments and
/// `exit` die with the child). A loop control the body raises (`break`,
/// `continue`) is the same `Ok` arm — it dies with the child too; only the
/// status is reported. An error of the body's own is the caller's report arm.
pub(super) fn run(
    text: &ScriptText,
    cmdline: &CommandLine,
    body: &ShortCStr,
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<ChildProcessError>> {
    crate::function_call::call(text, cmdline, body, cell)
        .map(|_| last_status_code(cell))
        .change_context(ChildProcessError::UserFunctionFailed)
}

/// The child's own last status, the code `sys::exit` carries
/// (`async_job::last_status_code` is the model). An un-borrowable state is an
/// invariant breach with no child-side report channel: status 1.
fn last_status_code(cell: &ForkCell<ShellState>) -> i32 {
    cell.borrow()
        .map(|state| state.last_status.exit_code())
        .unwrap_or(1)
}
