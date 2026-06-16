use crate::error::parse::{ParseErrorInfo, to_parse_err};
use crate::parse::if_block::IfBlock;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

pub(crate) fn run_if(ifblock: &IfBlock, cell: &ForkCell<ShellState>) -> Result<(), ParseErrorInfo> {
    let cond = ifblock.condition.as_bytes().map_err(to_parse_err)?;
    crate::repl::run_cond_list(cond, cell)?;
    let exit_code = {
        let state = cell.borrow().map_err(to_parse_err)?;
        state.last_status.exit_code()
    };
    if exit_code == 0 {
        let then = ifblock.then_body.as_bytes().map_err(to_parse_err)?;
        crate::repl::run_script(then, cell)?;
    } else {
        let mut done = false;
        for (elif_cond, elif_body) in &ifblock.elifs {
            let ec = elif_cond.as_bytes().map_err(to_parse_err)?;
            crate::repl::run_cond_list(ec, cell)?;
            let ec_exit = {
                let state = cell.borrow().map_err(to_parse_err)?;
                state.last_status.exit_code()
            };
            if ec_exit == 0 {
                let eb = elif_body.as_bytes().map_err(to_parse_err)?;
                crate::repl::run_script(eb, cell)?;
                done = true;
                break;
            }
        }
        if !done {
            if let Some(ref else_body) = ifblock.else_body {
                let eb = else_body.as_bytes().map_err(to_parse_err)?;
                crate::repl::run_script(eb, cell)?;
            } else {
                let mut state = cell.borrow_mut().map_err(to_parse_err)?;
                state.last_status = WaitStatus::Exited(0);
            }
        }
    }
    Ok(())
}
