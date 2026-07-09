use crate::loop_control::LoopControl;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::parse::if_block::IfBlock;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

pub(crate) fn run_if(
    ifblock: &IfBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let cond = ifblock
        .condition
        .as_bytes()
        .change_context(CmdError::Exec)?;
    crate::repl::run_cond_list(cond, cell)?;
    let exit_code = {
        let state = cell.borrow().change_context(CmdError::Exec)?;
        state.last_status.exit_code()
    };
    if exit_code == 0 {
        let then = ifblock
            .then_body
            .as_bytes()
            .change_context(CmdError::Exec)?;
        return crate::repl::run_script(then, cell);
    }
    for (elif_cond, elif_body) in &ifblock.elifs {
        let ec = elif_cond.as_bytes().change_context(CmdError::Exec)?;
        crate::repl::run_cond_list(ec, cell)?;
        let ec_exit = {
            let state = cell.borrow().change_context(CmdError::Exec)?;
            state.last_status.exit_code()
        };
        if ec_exit == 0 {
            let eb = elif_body.as_bytes().change_context(CmdError::Exec)?;
            return crate::repl::run_script(eb, cell);
        }
    }
    if let Some(ref else_body) = ifblock.else_body {
        let eb = else_body.as_bytes().change_context(CmdError::Exec)?;
        return crate::repl::run_script(eb, cell);
    } else {
        let mut state = cell.borrow_mut().change_context(CmdError::Exec)?;
        state.last_status = WaitStatus::Exited(0);
    }
    Ok(None)
}
