use crate::error::parse::{ParseErrorInfo, to_parse_err};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::state::ShellState;

pub(crate) fn run_loop(
    cond: &ShortCStr,
    body: &ShortCStr,
    invert: bool,
    cell: &ForkCell<ShellState>,
) -> Result<(), ParseErrorInfo> {
    loop {
        crate::repl::run_cond_list(cond.as_bytes().map_err(to_parse_err)?, cell)?;
        let exit_code = {
            let state = cell.borrow().map_err(to_parse_err)?;
            state.last_status.exit_code()
        };
        if (exit_code == 0) != invert {
            break;
        }
        crate::repl::run_script(body.as_bytes().map_err(to_parse_err)?, cell)?;
    }
    Ok(())
}
