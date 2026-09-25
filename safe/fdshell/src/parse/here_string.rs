use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use error_stack::{Report, bail};
use sys::ShortCStr;

/// Parse a here-string token at `i`: `<<<word`, or a bare `<<<` taking the
/// next token as its word. Quoted tokens are never here-strings. Returns the
/// redirect and how many following tokens the operator consumes (1 for the
/// bare form).
pub fn parse_here_string(
    tokens: &[Token],
    i: usize,
) -> Result<Option<(RedirectDef, usize)>, Report<ParseError>> {
    let Some((t, _start, _end, fq, _mask)) = tokens.get(i) else {
        return Ok(None);
    };
    if *fq || !t.starts_with(b"<<<") {
        return Ok(None);
    }
    // `<<<""` is one word whose quoted part contributes no bytes: its word is
    // the empty quoted region, not the next token. A bare `<<<` takes the next
    // token as its word; at end of input the word is empty.
    let (word, extra) = if t.len() > 3 {
        (t.get(3..).ok_or(ParseError::Never)?.clone(), 0)
    } else if _end - _start > t.len() {
        (ShortCStr::new(), 0)
    } else {
        match tokens.get(i + 1) {
            Some(next)
                if next.0.starts_with(b"<")
                    || next.0.starts_with(b">")
                    || next.0.starts_with(b"&")
                    || next.0.starts_with(b"%") =>
            {
                bail!(ParseError::InvalidRedirect);
            }
            Some(next) => (next.0.clone(), 1),
            None => (ShortCStr::new(), 0),
        }
    };
    Ok(Some((RedirectDef::here_string(word), extra)))
}

/// The token index of every here-string word: the operator token in the
/// attached form (`<<<word`, `<<<""`) and the following token in the bare
/// `<<<` form.
pub(crate) fn word_indices(tokens: &[Token]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for (i, (t, start, end, fq, _mask)) in tokens.iter().enumerate() {
        if *fq || !t.starts_with(b"<<<") {
            continue;
        }
        if t.len() > 3 || end - start > t.len() {
            out.push(i);
        } else if tokens.get(i + 1).is_some() {
            out.push(i + 1);
        }
    }
    out
}

#[cfg(test)]
mod tests;
