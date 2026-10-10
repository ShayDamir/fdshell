//! The parent-side handlers of a command: a user-function call, or an
//! intercepted in-process builtin. POSIX #2.4: the command's redirections apply
//! to it, and the shell's fds are put back when it finishes.

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// `None`: neither handler ran, so the caller takes the forked launch path (which
/// applies its own redirections in the child); `Some(control)`: the command ran,
/// with the loop control to propagate.
pub(crate) fn run_parent(
    text: &ScriptText,
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<Option<Option<LoopControl>>, Report<CmdError>> {
    let body = crate::function_call::look_up(cmdline, cell)?;
    let name = cmdline.command.as_bytes().change_context(CmdError::Never)?;
    if body.is_none() && !crate::intercept::commands::is_intercepted(name) {
        return Ok(None);
    }
    // The scope opens before the handler, so a redirection-open failure fails the
    // command without running it (bash `cd /nope/x >f` → rc 1, no `cd`).
    // `exec`/`become` replace the process image, so they opt out of the restore.
    let scope = if body.is_some() || crate::intercept::commands::is_in_process(name) {
        Some(crate::redirect::Scope::open(&cmdline.redirects, cell)?)
    } else {
        None
    };
    let result = match &body {
        Some(body) => crate::function_call::call(text, cmdline, body, cell).map(Some),
        None => crate::intercept::try_intercept(text, cmdline, cell),
    };
    match result {
        Ok(control) => {
            if let Some(scope) = scope {
                scope.restore()?;
            }
            Ok(control)
        }
        // The handler's report is the actionable error; the restore is
        // best-effort, the same rule as `run_env::restore` on the error path.
        Err(report) => {
            if let Some(scope) = scope {
                let _ = scope.restore();
            }
            Err(report)
        }
    }
}
