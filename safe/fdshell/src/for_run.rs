pub(crate) mod arith;
mod array_word;
mod word;

use crate::error::cmd::CmdError;
use crate::loop_control::{AtLevel, LoopControl, at_level, in_loop};
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
    // The redirections written after `done` belong to the whole loop (task
    // #298): the scope opens **before** the word list is expanded (bash's
    // order, measured), so a failing open leaves a `$(touch m)` word list
    // unexpanded, and it restores on every exit path (`Scope::run`).
    crate::redirect::Scope::open(&forblock.redirects, cell)?
        .run(|| in_loop(cell, || run_for_body(forblock, text, cell)))
}

fn run_for_body(
    forblock: &ForBlock,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.set_last_exit(0);
    }
    let words = crate::expand::expand_for_words(
        &forblock.words,
        &forblock.words_mask,
        &forblock.words_span,
        &forblock.words_quoted,
        text,
        cell,
    )
    .change_context(CmdError::Resolve)?;
    for word in &words {
        if let Some(control) = word::run_for_word(forblock, word, cell)? {
            match at_level(control) {
                AtLevel::Stop => break,
                // The for list's next iteration is the next word.
                AtLevel::Next => {}
                AtLevel::Outer(rest) => return Ok(Some(rest)),
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
