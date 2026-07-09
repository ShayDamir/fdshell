use crate::loop_control::LoopControl;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

use crate::error::cmd::CmdError;
use crate::parse::for_block::ForBlock;
use crate::state::ShellState;

pub(crate) fn run_for(
    forblock: &ForBlock,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    {
        let mut state = cell.borrow_mut().change_context(CmdError::Exec)?;
        state.last_status = WaitStatus::Exited(0);
    }
    let words =
        crate::expand::expand_for_words(&forblock.words, cell).change_context(CmdError::Resolve)?;
    for word in &words {
        {
            let mut state = cell.borrow_mut().change_context(CmdError::Exec)?;
            state.strings.insert(forblock.var.clone(), word.clone());
        }
        if let Some(control) = crate::repl::run_script(
            forblock.body.as_bytes().change_context(CmdError::Exec)?,
            cell,
        )? {
            match control {
                LoopControl::Break => break,
                LoopControl::Continue => continue,
            }
        }
    }
    Ok(())
}
