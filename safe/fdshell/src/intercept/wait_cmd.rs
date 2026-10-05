use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

/// POSIX `wait [pid…]` — reap background tasks by pid (all tasks when no
/// argument) and set `$?` to the last reaped exit status.
pub(crate) fn run_wait(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept_no_builtin(line, "wait", cmdline)?;
    // Expand the args so `wait $!` sees the pid, not the literal `$!`.
    let substituted = crate::substitute::substitute_args(
        &cmdline.args,
        &cmdline.args_mask,
        &cmdline.args_quoted,
        cell,
    )
    .change_context(CmdError::Resolve)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.last_status =
        crate::task::posix_wait(&substituted, &mut state).change_context(CmdError::Task)?;
    Ok(true)
}
