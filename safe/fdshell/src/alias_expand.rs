mod expand_at;

use crate::error::cmd::CmdError;
use crate::parse::{Token, pipe_token_span, token::tokenize_statement};
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// Expand aliases at every command position of the line — the first word and
/// the word after each `|` — chaining up to `MAX_ALIAS_DEPTH` per position,
/// when `expand_aliases` is on.
pub(crate) fn expand_alias(
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<ScriptText, Report<CmdError>> {
    let state = cell.borrow().change_context(CmdError::Never)?;
    if state.options & crate::options::EXPAND_ALIASES == 0 {
        return Ok(text.clone());
    }
    drop(state);
    let line = text.as_bytes().change_context(CmdError::Never)?;
    let tokens = tokenize_statement(line).change_context(CmdError::Parse)?;
    let mut current = ShortCStr::from_vec(line.to_vec()).change_context(CmdError::Never)?;
    let mut delta: isize = 0;
    for position in command_positions(line, &tokens) {
        expand_at::expand_position(line, position, &mut current, &mut delta, cell)?;
    }
    Ok(ScriptText::new(current, text.start, text.origin.clone()))
}

/// The command words of the line: the first token and the token following
/// each `|` token, as `(word, start, end, fully_quoted)`. The pipe test is the
/// span rule (`parse::redirect::pipe_token_span`), never the token text, so a
/// quoted `"|"` word is no command position (task #261).
fn command_positions<'a>(
    line: &[u8],
    tokens: &'a [Token],
) -> alloc::vec::Vec<(&'a ShortCStr, &'a usize, &'a usize, &'a bool)> {
    let mut out = alloc::vec::Vec::new();
    if let Some(first) = tokens.first() {
        out.push((&first.0, &first.1, &first.2, &first.3));
    }
    for (i, (_t, start, end, _fq, _mask, _spans)) in tokens.iter().enumerate() {
        if pipe_token_span(line, *start, *end)
            && let Some(n) = tokens.get(i + 1)
        {
            out.push((&n.0, &n.1, &n.2, &n.3));
        }
    }
    out
}

#[cfg(test)]
mod tests;
