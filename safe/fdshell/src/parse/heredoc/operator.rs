//! Token-level `<<` operator detection for heredoc parsing — the token
//! counterpart of the byte-level rule in `scan::heredoc::operator_delims`.

use super::super::Token;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, bail};
use sys::ShortCStr;

/// Whether the token at `i` is a heredoc operator: an unquoted `<<` word
/// (not `<<<`), not the command word, not in a pipeline position.
pub(crate) fn is_operator(tokens: &[Token], i: usize) -> bool {
    if i == 0 {
        return false;
    }
    let Some((t, _, _, fq, _)) = tokens.get(i) else {
        return false;
    };
    if *fq || !t.starts_with(b"<<") || t.starts_with(b"<<<") {
        return false;
    }
    !tokens
        .get(i - 1)
        .is_some_and(|(p, _, _, _, _)| p.eq_bytes(b"|"))
}

/// The number of heredoc operators in a token slice (a pipeline stage).
pub(crate) fn operator_count(tokens: &[Token]) -> usize {
    tokens
        .iter()
        .enumerate()
        .filter(|(i, _)| is_operator(tokens, *i))
        .count()
}

/// The token index of every heredoc delimiter: the operator token in the
/// attached form (`<<WORD`, `<<"Q"`, `<<""`) and the following token in the
/// bare `<<` form.
pub(crate) fn delimiter_token_indices(tokens: &[Token]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for (i, (t, start, end, _fq, _mask)) in tokens.iter().enumerate() {
        if !is_operator(tokens, i) {
            continue;
        }
        if attached(t, *start, *end) {
            out.push(i);
        } else if tokens.get(i + 1).is_some() {
            out.push(i + 1);
        }
    }
    out
}

/// The attached `<<` form: the raw span is longer than the unquoted word.
fn attached(t: &ShortCStr, start: usize, end: usize) -> bool {
    t.len() > 2 || end - start > t.len()
}

/// The `(raw delimiter span, quoted)` of every operator, in token order.
pub(super) fn operators<'a>(
    line: &'a [u8],
    tokens: &'a [Token],
) -> Result<Vec<(&'a [u8], bool)>, Report<ParseError>> {
    let mut found = Vec::new();
    for i in 1..tokens.len() {
        if !is_operator(tokens, i) {
            continue;
        }
        let (raw, _extra) = operator_raw(line, tokens, i)?;
        found.push(crate::scan::heredoc::delimiter_word(raw));
    }
    Ok(found)
}

/// The raw delimiter span of the operator at `i` and how many following
/// tokens it consumes (1 for the bare `<<` form).
pub(super) fn operator_raw<'a>(
    line: &'a [u8],
    tokens: &'a [Token],
    i: usize,
) -> Result<(&'a [u8], usize), Report<ParseError>> {
    let Some((t, start, end, _fq, _mask)) = tokens.get(i) else {
        bail!(ParseError::Never);
    };
    if attached(t, *start, *end) {
        // Attached form: `<<WORD`, `<<"Q"`, `<<""` — the raw span after `<<`.
        // The token offsets are within the line by construction.
        Ok((line.get(*start + 2..*end).ok_or(ParseError::Never)?, 0))
    } else {
        // Bare `<<`: the next token is the delimiter word.
        let Some(next) = tokens.get(i + 1) else {
            bail!(ParseError::InvalidRedirect);
        };
        if invalid_delimiter(&next.0) {
            bail!(ParseError::InvalidRedirect);
        }
        // The token offsets are within the line by construction.
        Ok((line.get(next.1..next.2).ok_or(ParseError::Never)?, 1))
    }
}

/// A bare `<<` delimiter word may not be a separator or another operator.
/// A `)` never forms a token (the tokenizer emits a `;` separator instead),
/// so the `;` arm covers the `<<`-before-`)` case.
fn invalid_delimiter(word: &ShortCStr) -> bool {
    word.is_empty()
        || word.starts_with(b"<")
        || word.starts_with(b">")
        || word.starts_with(b"&")
        || word.starts_with(b"%")
        || word.eq_bytes(b";")
        || word.eq_bytes(b"|")
}
