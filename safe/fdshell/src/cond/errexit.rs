use crate::error::cmd::CmdError;
use crate::options;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

/// Whether errexit stops the shell now: the option is on and the last command
/// of the list failed. The failing status stays in `last_status`, so the
/// shell's exit code is the failing command's code, as in bash.
pub(crate) fn should_exit(cell: &ForkCell<ShellState>) -> Result<bool, Report<CmdError>> {
    let state = cell.borrow().change_context(CmdError::Never)?;
    Ok(state.options & options::ERREXIT != 0 && state.last_status.exit_code() != 0)
}
