use crate::parse::if_block::IfBlock;
use crate::state::ShellState;
use sys::siginfo::WaitStatus;

pub(crate) fn run_if(ifblock: &IfBlock, state: &mut ShellState) -> Result<(), i32> {
    let cond = ifblock.condition.as_bytes()?;
    crate::repl::run_cond_list(cond, state)?;
    if state.last_status.exit_code() == 0 {
        let then = ifblock.then_body.as_bytes()?;
        crate::repl::run_script(then, state)?;
    } else {
        let mut done = false;
        for (elif_cond, elif_body) in &ifblock.elifs {
            let ec = elif_cond.as_bytes()?;
            crate::repl::run_cond_list(ec, state)?;
            if state.last_status.exit_code() == 0 {
                let eb = elif_body.as_bytes()?;
                crate::repl::run_script(eb, state)?;
                done = true;
                break;
            }
        }
        if !done {
            if let Some(ref else_body) = ifblock.else_body {
                let eb = else_body.as_bytes()?;
                crate::repl::run_script(eb, state)?;
            } else {
                state.last_status = WaitStatus::Exited(0);
            }
        }
    }
    Ok(())
}
