use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

use super::validation::*;

pub(crate) fn run_export_fd(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    check_captures_not_supported(line, "export_fd", &cmdline.captures)?;
    check_redirects_not_supported(line, "export_fd", &cmdline.redirects)?;

    let state = cell.borrow().change_context(CmdError::Exec)?;
    let status = match crate::child::fdpass::export_fd(&cmdline.args, &state) {
        Ok(_) => WaitStatus::Exited(0),
        Err(report) => WaitStatus::Exited(match report.current_context() {
            crate::error::fdpass::FdPassError::SendFailed => sys::errno::EIO,
            crate::error::fdpass::FdPassError::NotFound
            | crate::error::fdpass::FdPassError::InvalidName
            | crate::error::fdpass::FdPassError::MissingArg => sys::errno::EINVAL,
        }),
    };
    drop(state);

    let mut state = cell.borrow_mut().change_context(CmdError::Exec)?;
    state.last_status = status;
    Ok(true)
}
