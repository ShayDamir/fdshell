//! Token-level `<<` operator detection for heredoc parsing — the token
//! counterpart of the byte-level rule in `scan::heredoc::operator_delims`.

use super::super::Token;
use crate::error::parse::ParseError;
use crate::scan::heredoc::{Operator, attached, invalid_delimiter, operator_delim};
use alloc::vec::Vec;
use error_stack::{Report, bail};

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

/// The token index of every heredoc operator (never the command word).
fn operator_indices(tokens: &[Token]) -> Vec<usize> {
    (1..tokens.len())
        .filter(|&i| is_operator(tokens, i))
        .collect()
}

/// The number of heredoc operators in a token slice (a pipeline stage).
pub(crate) fn operator_count(tokens: &[Token]) -> usize {
    operator_indices(tokens).len()
}

/// The token index of every heredoc delimiter: the operator token in the
/// attached form (`<<WORD`, `<<-WORD`, `<<"Q"`, `<<""`) and the following
/// token in the bare `<<` / `<<-` form.
pub(crate) fn delimiter_token_indices(line: &[u8], tokens: &[Token]) -> Vec<usize> {
    operator_indices(tokens)
        .into_iter()
        .filter_map(|i| {
            let (_t, start, end, _fq, _mask) = tokens.get(i)?;
            if attached(line, *start, *end) {
                Some(i)
            } else {
                tokens.get(i + 1).is_some().then_some(i + 1)
            }
        })
        .collect()
}

/// The operator of every `<<` in token order.
pub(crate) fn operators(
    line: &[u8],
    tokens: &[Token],
) -> Result<Vec<Operator>, Report<ParseError>> {
    let mut found = Vec::new();
    for i in operator_indices(tokens) {
        let (op, _extra) = operator_at(line, tokens, i)?;
        found.push(op);
    }
    Ok(found)
}

/// The operator at `i`: its delimiter and how many following tokens it
/// consumes (1 for the bare `<<` / `<<-` form, whose delimiter is the next
/// word). The `<<-` marker is inherited by that word, so `<<- -` strips.
pub(super) fn operator_at(
    line: &[u8],
    tokens: &[Token],
    i: usize,
) -> Result<(Operator, usize), Report<ParseError>> {
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
