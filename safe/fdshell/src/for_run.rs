use crate::loop_control::LoopControl;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::parse::for_block::ForBlock;
use crate::state::ShellState;

pub(crate) fn run_for(
    forblock: &ForBlock,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.set_last_exit(0);
    }
    let words = crate::expand::expand_for_words(&forblock.words, text, cell)
        .change_context(CmdError::Resolve)?;
    for word in &words {
        {
            let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
            state.set_var(forblock.var.clone(), word.clone());
        }
        if let Some(control) = crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
            crate::repl::run_script(&forblock.body, cell)
        })? {
            match control {
                LoopControl::Break => break,
                LoopControl::Continue => continue,
                LoopControl::Return => return Ok(Some(LoopControl::Return)),
            }
        }
    }
    Ok(None)
}
