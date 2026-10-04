use crate::loop_control::LoopControl;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::parse::while_block::LoopBlock;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// The condition's heredoc bodies as byte vectors (for `run_cond_list`).
fn body_bytes(bodies: &[ShortCStr]) -> Vec<Vec<u8>> {
    bodies
        .iter()
        .filter_map(|b| b.as_bytes().ok().map(|bytes| bytes.to_vec()))
        .collect()
}

pub(crate) fn run_loop(
    block: &LoopBlock,
    invert: bool,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let cond_bodies = body_bytes(&block.cond_bodies);
    let mut ran_body = false;
    loop {
        crate::repl::run_cond_list(&block.condition, &cond_bodies, cell, true)?;
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
                LoopControl::Return => return Ok(Some(LoopControl::Return)),
                LoopControl::Exit => return Ok(Some(LoopControl::Exit)),
            }
        }
    }
    if !ran_body {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.set_last_exit(0);
    }
    Ok(None)
}
