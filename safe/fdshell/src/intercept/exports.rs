use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) fn run_export(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "export", cmdline)?;

    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    crate::exports::handle_export(&cmdline.args, &mut state)
        .change_context(CmdError::ExportName)?;
    state.set_last_exit(0);
    Ok(true)
}
