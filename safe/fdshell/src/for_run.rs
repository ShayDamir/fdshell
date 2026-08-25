mod array_word;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::parse::for_block::ForBlock;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

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
        if let Some(control) = run_for_word(forblock, word, cell)? {
            match control {
                LoopControl::Break => break,
                LoopControl::Continue => {}
                LoopControl::Return => return Ok(Some(LoopControl::Return)),
            }
        }
    }
    Ok(None)
}

/// One word of the for list: for an fd for-var (`%name`), an array reference
/// in the word list expands each entry into the for var as a dup; any other
/// word binds as a string.
fn run_for_word(
    forblock: &ForBlock,
    word: &sys::ImportedStr,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let dups = if forblock.var.starts_with(b"%") {
        array_word::dup_array_word(word, cell)?
    } else {
        None
    };
    match dups {
        Some(dups) => {
            let name = forblock
                .var
                .strip_prefix(b"%")
                .unwrap_or_else(|| forblock.var.clone());
            for fdvar in dups {
                {
                    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
                    state.set_fd_var(name.clone(), fdvar);
                }
                if let Some(control) = crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
                    crate::repl::run_script(&forblock.body, cell)
                })? {
                    return Ok(Some(control));
                }
            }
            Ok(None)
        }
        None => {
            {
                let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
                state.set_var(forblock.var.clone(), word.clone());
            }
            if let Some(control) = crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
                crate::repl::run_script(&forblock.body, cell)
            })? {
                Ok(Some(control))
            } else {
                Ok(None)
            }
        }
    }
}
