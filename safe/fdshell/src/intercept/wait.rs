use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) fn run_wait(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "wait", cmdline)?;

    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.last_status =
        crate::task::try_wait(&cmdline.args, &mut state).change_context(CmdError::Task)?;
    Ok(true)
}
