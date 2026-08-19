use crate::loop_control::LoopControl;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::parse::while_block::LoopBlock;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) fn run_loop(
    block: &LoopBlock,
    invert: bool,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let mut ran_body = false;
    loop {
        crate::repl::run_cond_list(&block.condition, cell)?;
        let exit_code = {
            let state = cell.borrow().change_context(CmdError::Never)?;
            state.last_status.exit_code()
        };
        if (exit_code == 0) != invert {
            break;
        }
        ran_body = true;
        if let Some(control) = crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
            crate::repl::run_script(&block.body, cell)
        })? {
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
