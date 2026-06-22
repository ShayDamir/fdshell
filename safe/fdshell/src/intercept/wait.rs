use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

use super::validation::*;

pub(crate) fn run_wait(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    check_builtin_not_supported(line, "wait", cmdline.builtin)?;
    check_captures_not_supported(line, "wait", &cmdline.captures)?;
    check_redirects_not_supported(line, "wait", &cmdline.redirects)?;

    let mut state = cell.borrow_mut().change_context(CmdError::Exec)?;
    state.last_status =
        crate::task::try_wait(&cmdline.args, &mut state).change_context(CmdError::Task)?;
    Ok(true)
}
