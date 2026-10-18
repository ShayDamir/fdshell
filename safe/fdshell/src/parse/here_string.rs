use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::redirect::RedirectDef;
use crate::scan::heredoc::{io_fd, string_span};
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

/// Parse a here-string token at `i`: `[N]<<<word`, or a bare `[N]<<<` taking
/// the next token as its word. The operator shape is read from the token's
/// **raw span** through the shared `string_span`, never from the token text:
/// quote removal makes `3"<<<"x` read exactly like `3<<<x`, and a token-text
/// rule for a raw-byte structure is the byte/token count mismatch (LESSONS,
/// task #276's `<<` twin). Returns the redirect (the word with **its own**
/// quote mask, task #269, on the `N<<<word` descriptor — 0 for the unprefixed
/// form) and how many following tokens the operator consumes (1 for the bare
/// form). Quoted tokens are never here-strings.
pub fn parse_here_string(
    line: &[u8],
    tokens: &[Token],
    i: usize,
) -> Result<Option<(RedirectDef, usize)>, Report<ParseError>> {
    let Some((t, start, end, fq, mask)) = tokens.get(i) else {
        return Ok(None);
    };
    if *fq {
        return Ok(None);
    }
    let Some((op, prefix)) = string_span(line, *start, *end) else {
        return Ok(None);
    };
    // `<<<""` is one word whose quoted part contributes no bytes: its word is
    // the empty quoted region, not the next token. A bare `<<<` takes the next
    // token as its word; at end of input the word is empty.
    let (word, mask, extra) = if has_attached_word(t.len(), prefix) {
        (
            t.get(prefix + 3..).ok_or(ParseError::Never)?.clone(),
            word_mask(mask, prefix),
            0,
        )
    } else if super::word_quoted::word_quoted(t, *start, *end) {
        (ShortCStr::new(), Vec::new(), 0)
    } else {
        match tokens.get(i + 1) {
            Some(next) if is_bare_operand(&next.0) => bail!(ParseError::InvalidRedirect),
            Some(next) => (next.0.clone(), next.4.clone(), 1),
            None => (ShortCStr::new(), Vec::new(), 0),
        }
    };
    let digits = line.get(*start..op).ok_or(ParseError::Never)?;
    let Some(export_to) = io_fd(digits) else {
        let prefix = ShortCStr::from_vec(digits.to_vec()).change_context(ParseError::Never)?;
        bail!(ParseError::InvalidHereStringFd { prefix });
    };
    Ok(Some((
        RedirectDef::here_string(export_to, word, mask),
        extra,
    )))
}

/// The here-string word's mask: the operator token's mask sliced past the
/// `[io-number] <<<` prefix. This is the **only** place that offset exists, so
/// the word and its mask cannot drift apart (LESSONS: slice the word, its mask
/// and its quoted flags with the same offset). The prefix digits are raw bytes
/// by construction — `string_span` only reports a span whose digits begin it —
/// so the token text's first `prefix + 3` bytes are 1:1 with the mask. A mask
/// shorter than the word degrades to unquoted, the same padding
/// `substitute/arg.rs` applies; the empty word of `<<<""` keeps the empty mask,
/// which `substitute::fully_quoted` reads as quoted.
pub(crate) fn word_mask(op_mask: &[bool], prefix: usize) -> Vec<bool> {
    op_mask.get(prefix + 3..).unwrap_or_default().to_vec()
}

/// Whether the operator token carries its own word: the token is longer than
/// its `prefix + 3` operator bytes (`3<<<x`, `3<<<""`), as opposed to the bare
/// `3<<<` form, which takes the next token. One owner for the test, so the
/// parse and the brace-protection consumer cannot drift.
fn has_attached_word(len: usize, prefix: usize) -> bool {
    len > prefix + 3
}

/// The bare form's operand guard: an operand may not start with an operator
/// byte (the same rule as the bare `<<` delimiter, `invalid_delimiter`, and the
/// bare `>`/`<` redirects — keep the three in lockstep; owned by task #323).
fn is_bare_operand(word: &ShortCStr) -> bool {
    word.starts_with(b"<")
        || word.starts_with(b">")
        || word.starts_with(b"&")
        || word.starts_with(b"%")
}

/// The token index of every here-string word: the operator token in the
/// attached form (`<<<word`, `3<<<word`, `<<<""`) and the following token in
/// the bare `<<<` form.
pub(crate) fn word_indices(line: &[u8], tokens: &[Token]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for (i, (t, start, end, _fq, _mask)) in tokens.iter().enumerate() {
        let Some((_op, prefix)) = string_span(line, *start, *end) else {
            continue;
        };
        if has_attached_word(t.len(), prefix) || super::word_quoted::word_quoted(t, *start, *end) {
            out.push(i);
        } else if tokens.get(i + 1).is_some() {
            out.push(i + 1);
        }
    }
    out
}

#[cfg(test)]
mod tests;
