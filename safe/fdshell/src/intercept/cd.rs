use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

use super::validation::*;

pub(crate) fn run_cd(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    check_builtin_not_supported(line, "cd", cmdline.builtin)?;
    check_captures_not_supported(line, "cd", &cmdline.captures)?;
    check_redirects_not_supported(line, "cd", &cmdline.redirects)?;

    let mut state = cell.borrow_mut().change_context(CmdError::Exec)?;
    crate::cd::cd(&cmdline.args, &mut state).change_context(CmdError::Cd)?;
    state.last_status = WaitStatus::Exited(0);
    Ok(true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
