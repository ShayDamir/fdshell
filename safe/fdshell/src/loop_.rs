use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::state::ShellState;

pub(crate) fn run_loop(
    cond: &ShortCStr,
    body: &ShortCStr,
    invert: bool,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    loop {
        crate::repl::run_cond_list(cond.as_bytes().change_context(CmdError::Exec)?, cell)?;
        let exit_code = {
            let state = cell.borrow().change_context(CmdError::Exec)?;
            state.last_status.exit_code()
        };
        if (exit_code == 0) != invert {
            break;
        }
        crate::repl::run_script(body.as_bytes().change_context(CmdError::Exec)?, cell)?;
    }
    Ok(())
}
