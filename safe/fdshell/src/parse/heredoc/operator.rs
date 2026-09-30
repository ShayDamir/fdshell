//! Token-level `<<` operator detection for heredoc parsing — the token
//! counterpart of the byte-level rule in `scan::heredoc::operator_delims`.

use super::super::Token;
use crate::error::parse::ParseError;
use crate::scan::heredoc::{Operator, operator_delim};
use alloc::vec::Vec;
use error_stack::{Report, bail};
use sys::ShortCStr;

/// Whether the token at `i` is a heredoc operator: an unquoted `<<` word
/// (not `<<<`) — the `<<-` tab-stripping form included — not the command
/// word, not in a pipeline position.
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
/// attached form (`<<WORD`, `<<-WORD`, `<<"Q"`, `<<""`) and the following
/// token in the bare `<<` / `<<-` form.
pub(crate) fn delimiter_token_indices(line: &[u8], tokens: &[Token]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for (i, (_t, start, end, _fq, _mask)) in tokens.iter().enumerate() {
        if !is_operator(tokens, i) {
            continue;
        }
        if attached(line, *start, *end) {
            out.push(i);
        } else if tokens.get(i + 1).is_some() {
            out.push(i + 1);
        }
    }
    out
}

/// The attached `<<` form: a delimiter byte follows `<<` (after an optional
/// `<<-` marker). `<<` and `<<-` alone are both bare: their delimiter is the
/// next word, read by the same raw-byte rule the parser uses.
fn attached(line: &[u8], start: usize, end: usize) -> bool {
    !operator_delim(line, start, end).1.is_empty()
}

/// The operator of every `<<` in token order.
pub(super) fn operators<'a>(
    line: &'a [u8],
    tokens: &'a [Token],
) -> Result<Vec<Operator<'a>>, Report<ParseError>> {
    let mut found = Vec::new();
    for i in 1..tokens.len() {
        if !is_operator(tokens, i) {
            continue;
        }
        let (op, _extra) = operator_at(line, tokens, i)?;
        found.push(op);
    }
    Ok(found)
}

/// The operator at `i`: its delimiter and how many following tokens it
/// consumes (1 for the bare `<<` / `<<-` form, whose delimiter is the next
/// word). The `<<-` marker is inherited by that word, so `<<- -` strips.
pub(super) fn operator_at<'a>(
    line: &'a [u8],
    tokens: &'a [Token],
    i: usize,
) -> Result<(Operator<'a>, usize), Report<ParseError>> {
    let Some((_t, start, end, _fq, _mask)) = tokens.get(i) else {
        bail!(ParseError::Never);
    };
    let (strip, delim) = operator_delim(line, *start, *end);
    if !delim.is_empty() {
        // Attached form: `<<WORD`, `<<-WORD`, `<<"Q"`, `<<""`.
        return Ok((Operator::new(delim, strip), 0));
    }
    // Bare `<<` (or `<<-` with a separate word): the next token is the
    // delimiter word; the marker is inherited, so `<<- -` strips tabs.
    let Some(next) = tokens.get(i + 1) else {
        bail!(ParseError::InvalidRedirect);
    };
    if invalid_delimiter(&next.0) {
        bail!(ParseError::InvalidRedirect);
    }
    let raw = line.get(next.1..next.2).ok_or(ParseError::Never)?;
    Ok((Operator::new(raw, strip), 1))
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
