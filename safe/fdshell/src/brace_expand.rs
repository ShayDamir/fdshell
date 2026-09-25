//! Brace expansion (`{a,b}`, `{1..5}`): a pre-parse rewrite that replaces
//! every brace group with its expanded words, matching bash. The expansion
//! runs on raw token spans (quotes included); the output is re-tokenized by
//! the normal parse, so empty expanded words drop out like any other
//! unquoted empty word. See the README ("Brace expansion") for the
//! documented deviations.

mod gobbler;
mod protect;
mod seq;
mod word;

use crate::error::cmd::CmdError;
use crate::error::parse::ParseError;
use crate::parse::{Token, token::tokenize_statement};
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::ShortCStr;

#[cfg(test)]
mod tests;

/// Brace-expand the line's words; a no-op (fast path) when the line has no
/// `{` byte.
pub(crate) fn expand(text: &ScriptText) -> Result<ScriptText, Report<CmdError>> {
    let line = text.as_bytes().change_context(CmdError::Never)?;
    if !line.contains(&b'{') {
        return Ok(text.clone());
    }
    let tokens = tokenize_statement(line).change_context(CmdError::Parse)?;
    let data = rebuild(line, &tokens).change_context(CmdError::Parse)?;
    Ok(ScriptText::new(data, text.start, text.origin.clone()))
}

/// Rebuild the line byte-exact except for the expanded words.
fn rebuild(line: &[u8], tokens: &[Token]) -> Result<ShortCStr, Report<ParseError>> {
    let protected = protect::protected(line, tokens);
    let mut out: Vec<u8> = Vec::with_capacity(line.len());
    let mut pos = 0usize;
    for (i, (_t, start, end, _fq, _mask)) in tokens.iter().enumerate() {
        let Some(span) = line.get(*start..*end) else {
            return Err(Report::new(ParseError::Never));
        };
        if let Some(gap) = line.get(pos..*start) {
            out.extend_from_slice(gap);
        }
        if protected.contains(&i) || !span.contains(&b'{') {
            out.extend_from_slice(span);
        } else if let Some(words) = word::expand_word(span)? {
            append_words(words, &mut out);
        } else {
            // A `{` with no valid group: the word is unchanged.
            out.extend_from_slice(span);
        }
        pos = *end;
    }
    if let Some(tail) = line.get(pos..) {
        out.extend_from_slice(tail);
    }
    // `out` is built from the NUL-free input line plus spaces, so a NUL here
    // is a logic error, not a user-input error.
    ShortCStr::from_vec(out).change_context(ParseError::Never)
}

/// Append one token's expanded words to `out`, joined by single spaces.
/// Empty words contribute no bytes: the re-tokenization drops them like any
/// unquoted empty word, exactly like bash's word splitting.
fn append_words(words: Vec<Vec<u8>>, out: &mut Vec<u8>) {
    for w in words {
        if !out.is_empty() && !out.ends_with(b" ") {
            out.push(b' ');
        }
        out.extend_from_slice(&w);
    }
}
