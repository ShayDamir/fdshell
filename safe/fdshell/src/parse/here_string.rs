use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use error_stack::{Report, bail};
use sys::ShortCStr;

/// Parse a here-string token at `i`: `<<<word`, or a bare `<<<` taking the
/// next token as its word. Quoted tokens are never here-strings. Returns the
/// redirect (the word with **its own** quote mask, task #269) and how many
/// following tokens the operator consumes (1 for the bare form).
pub fn parse_here_string(
    tokens: &[Token],
    i: usize,
) -> Result<Option<(RedirectDef, usize)>, Report<ParseError>> {
    let Some((t, start, end, fq, mask)) = tokens.get(i) else {
        return Ok(None);
    };
    if *fq || !t.starts_with(b"<<<") {
        return Ok(None);
    }
    // `<<<""` is one word whose quoted part contributes no bytes: its word is
    // the empty quoted region, not the next token. A bare `<<<` takes the next
    // token as its word; at end of input the word is empty.
    let (word, mask, extra) = if t.len() > 3 {
        (
            t.get(3..).ok_or(ParseError::Never)?.clone(),
            word_mask(mask),
            0,
        )
    } else if super::word_quoted::word_quoted(t, *start, *end) {
        (ShortCStr::new(), Vec::new(), 0)
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
            Some(next) => (next.0.clone(), next.4.clone(), 1),
            None => (ShortCStr::new(), Vec::new(), 0),
        }
    };
    Ok(Some((RedirectDef::here_string(word, mask), extra)))
}

/// The here-string word's mask: the operator token's mask sliced at the
/// `<<<` prefix (offset 3). This is the **only** place that offset exists, so
/// the word and its mask cannot drift apart (LESSONS: slice the word, its mask
/// and its quoted flags with the same offset). A mask shorter than the word
/// degrades to unquoted, the same padding `substitute/arg.rs` applies; the
/// empty word of `<<<""` keeps the empty mask, which `substitute::fully_quoted`
/// reads as quoted.
pub(crate) fn word_mask(op_mask: &[bool]) -> Vec<bool> {
    op_mask.get(3..).unwrap_or_default().to_vec()
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
        if t.len() > 3 || super::word_quoted::word_quoted(t, *start, *end) {
            out.push(i);
        } else if tokens.get(i + 1).is_some() {
            out.push(i + 1);
        }
    }
    out
}

#[cfg(test)]
mod tests;
