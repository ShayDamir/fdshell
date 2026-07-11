use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

pub(crate) fn run_cd(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "cd", cmdline)?;

    let mut state = cell.borrow_mut().change_context(CmdError::Exec)?;
    crate::cd::cd(&cmdline.args, &mut state).change_context(CmdError::Cd)?;
    state.last_status = WaitStatus::Exited(0);
    Ok(true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
