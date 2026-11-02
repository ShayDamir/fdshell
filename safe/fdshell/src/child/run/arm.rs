//! The dispatch half of `child::run`: which arm answers the name, decided once
//! (bash's order: the forked table, then a user function, then the intercepted
//! table, then `PATH`) so that the trace goes to the arm that runs it.

use crate::child::{self, Command, external};
use crate::error::child_process::ChildProcessError;
use crate::parse::{BuiltinPrefix, CommandLine};
use crate::state::ShellState;
use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// Run the arm of a child whose setup is done: `resolved` holds the substituted
/// words, the scoped prefix has been applied and the redirects exported.
pub(super) fn run(
    cmd: &Command,
    resolved: &[ShortCStr],
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
    stage_text: Option<&ScriptText>,
) -> Result<i32, Report<ChildProcessError>> {
    let args = &cmdline.args;
    let sealed: Vec<sys::ExportedCStr> = resolved.iter().map(|cs| cs.export()).collect();
    let refs: Vec<&CStr> = sealed.iter().map(|rc| rc.as_ref()).collect();

    let state = cell
        .borrow()
        .change_context(ChildProcessError::BorrowFailed)?;
    // `run_external` re-borrows the cell to glob the command name, so the
    // trace borrow must end first (RefCell, LESSONS.md). `command NAME`
    // dispatches as a builtin only when the name is in the dispatch table;
    // otherwise it falls through to the external (PATH) search (POSIX #6.6).
    let builtin = match cmd.prefix {
        BuiltinPrefix::Builtin => true,
        BuiltinPrefix::Command => child::dispatch::is_dispatched(&cmd.name),
        BuiltinPrefix::None => child::dispatch::builtin_first(&cmd.name, &state),
    };
    drop(state);

    // Which arm answers the name, decided once (bash's order: the forked table,
    // then a user function, then the intercepted table, then `PATH`) so that the
    // trace below goes to the arm that runs it.
    let stage = stage_text.is_some();
    let fn_body = if builtin || !stage {
        None
    } else {
        crate::function_call::look_up(cmdline, cell)
            .change_context(ChildProcessError::BorrowFailed)?
    };
    let intercepts = child::intercepted::answers(&cmd.name, builtin, stage, &fn_body);
    // The intercept arm owns its trace: `intercept::try_intercept` prints the
    // raw arguments (and `set` traces itself), which is what the plain path does
    // for the same command — one line per command, on both paths.
    if !intercepts {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::BorrowFailed)?;
        crate::xtrace::trace(cmd.name.as_bytes().unwrap_or(&[]), resolved, &state);
    }

    if builtin {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::BorrowFailed)?;
        child::dispatch::run_builtin(cmd.name.clone(), &refs, args, &state)
    } else if let Some(body) = fn_body
        && let Some(text) = stage_text
    {
        // A stage is a full command, so its name resolves through the same
        // order a plain command uses: user function first, then both builtin
        // tables, then `PATH`. `function_call::look_up` is the single owner of
        // the rule (it answers `None` when a `builtin`/`command` prefix is set,
        // so those rows keep reporting). The arms sit at this decision point so
        // they run after everything `child_main` owns: the capture socket, the
        // stage's redirects, arg substitution, the scoped prefix and the trace.
        child::user_function::run(text, cmdline, &body, cell)
    } else if intercepts && let Some(text) = stage_text {
        // The intercepted table's arm (task #381): the plain path runs these
        // handlers in the parent, and the stage's fork is the child's own copy
        // of the state — bash's stage subshell, where a `cd`, an option, a
        // positional or an alias change dies with the fork.
        child::intercepted::run(text, cmdline, cell)
    } else {
        external::run_external(cmd, &refs, cell)
    }
}
