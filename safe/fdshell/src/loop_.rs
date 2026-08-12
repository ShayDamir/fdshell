use crate::loop_control::LoopControl;
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
    let mut ran_body = false;
    loop {
        crate::repl::run_cond_list(cond.as_bytes().change_context(CmdError::Never)?, cell)?;
        let exit_code = {
            let state = cell.borrow().change_context(CmdError::Never)?;
            state.last_status.exit_code()
        };
        if (exit_code == 0) != invert {
            break;
        }
        ran_body = true;
        if let Some(control) =
            crate::repl::run_script(body.as_bytes().change_context(CmdError::Never)?, cell)?
        {
            match control {
                LoopControl::Break => break,
                LoopControl::Continue => continue,
            }
        }
    }
    if !ran_body {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.set_last_exit(0);
    }
    Ok(())
}
