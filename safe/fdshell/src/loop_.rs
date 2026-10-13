use crate::loop_control::{AtLevel, LoopControl, at_level, in_loop};
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
    in_loop(cell, || run_loop_body(block, invert, cell))
}

fn run_loop_body(
    block: &LoopBlock,
    invert: bool,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let cond_bodies = body_bytes(&block.cond_bodies);
    let mut ran_body = false;
    loop {
        // A `break`/`continue` raised inside the condition (an `if` then-body, a
        // `{ …; }` group, a `case` arm, a function called from it) is the loop's
        // own signal: the condition runs inside this loop's level (`in_loop`), so
        // it is consumed here exactly like the body's.
        if let Some(control) =
            crate::repl::run_cond_list(&block.condition, &cond_bodies, cell, true)?
        {
            match at_level(control) {
                AtLevel::Stop => break,
                // The loop's next iteration is the next condition test, body skipped:
                // a `continue` in a condition re-tests it, an infinite loop exactly as
                // in bash (README "Loop control").
                AtLevel::Next => continue,
                AtLevel::Outer(rest) => return Ok(Some(rest)),
            }
        }
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
            match at_level(control) {
                AtLevel::Stop => break,
                AtLevel::Next => continue,
                AtLevel::Outer(rest) => return Ok(Some(rest)),
            }
        }
    }
    if !ran_body {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.set_last_exit(0);
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
