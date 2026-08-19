use crate::loop_control::LoopControl;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::parse::if_block::IfBlock;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) fn run_if(
    ifblock: &IfBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    crate::repl::run_cond_list(&ifblock.condition, cell)?;
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
        crate::repl::run_cond_list(&arm.cond, cell)?;
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
