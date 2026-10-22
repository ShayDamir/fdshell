use crate::loop_control::LoopControl;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::parse::if_block::IfBlock;
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

pub(crate) fn run_if(
    ifblock: &IfBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    // The redirections written after `fi` belong to the whole `if` (task #298):
    // the scope opens before the condition runs, so a failing open aborts the
    // command, and it restores on every exit path (`Scope::run`).
    crate::redirect::Scope::open(&ifblock.redirects, cell)?.run(|| run_if_body(ifblock, cell))
}

/// The `if` proper: the condition, the taken arm's body, the `elif` chain, and
/// the `else` body, all in the caller's shell.
fn run_if_body(
    ifblock: &IfBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let cond_bodies = body_bytes(&ifblock.cond_bodies);
    let control = crate::repl::run_cond_list(
        &ifblock.condition,
        &cond_bodies,
        cell,
        true,
        ifblock.cond_background,
    )?;
    if let Some(control) = control {
        // An `if` is not a loop level (README "Loop control"), so the signal is
        // the caller's: propagate it whole, consuming nothing.
        return Ok(Some(control));
    }
    let exit_code = {
        let state = cell.borrow().change_context(CmdError::Never)?;
        state.last_status.exit_code()
    };
    if exit_code == 0 {
        return crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
            crate::repl::run_script(&ifblock.then_body, cell)
        });
    }
    for arm in &ifblock.elifs {
        let arm_bodies = body_bytes(&arm.cond_bodies);
        let control =
            crate::repl::run_cond_list(&arm.cond, &arm_bodies, cell, true, arm.cond_background)?;
        if let Some(control) = control {
            return Ok(Some(control));
        }
        let ec_exit = {
            let state = cell.borrow().change_context(CmdError::Never)?;
            state.last_status.exit_code()
        };
        if ec_exit == 0 {
            return crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
                crate::repl::run_script(&arm.body, cell)
            });
        }
    }
    if let Some(ref else_body) = ifblock.else_body {
        return crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
            crate::repl::run_script(else_body, cell)
        });
    } else {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.set_last_exit(0);
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
