//! Token-level `<<` operator detection for heredoc parsing — the token
//! counterpart of the byte-level rule in `scan::heredoc::operator_delims`.

use super::super::Token;
use crate::error::parse::ParseError;
use crate::scan::heredoc::{
    Operator, attached, invalid_delimiter, io_fd, operator_delim, span_operator,
};
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

/// Whether the token at `i` is a heredoc operator: an unquoted `<<` word
/// (not `<<<`) with the io-number form `N<<word` included, never the command
/// word, never in a pipeline position. The shape test reads the token's **raw
/// span** and the shared `span_operator`, never the token text: quote removal
/// makes `3"<<"T` read exactly like `3<<T`, and a token-text rule for a
/// raw-byte structure is the byte/token count mismatch (LESSONS).
pub(crate) fn is_operator(line: &[u8], tokens: &[Token], i: usize) -> bool {
    if i == 0 {
        return false;
    }
    let Some((_t, start, end, fq, _mask, _spans)) = tokens.get(i) else {
        return false;
    };
    if *fq || span_operator(line, *start, *end).is_none() {
        return false;
    }
    !tokens
        .get(i - 1)
        .is_some_and(|(p, _, _, _, _, _)| p.eq_bytes(b"|"))
}

/// The token index of every heredoc operator (never the command word).
fn operator_indices(line: &[u8], tokens: &[Token]) -> Vec<usize> {
    (1..tokens.len())
        .filter(|&i| is_operator(line, tokens, i))
        .collect()
}

/// The number of heredoc operators in a token slice (a pipeline stage).
pub(crate) fn operator_count(line: &[u8], tokens: &[Token]) -> usize {
    operator_indices(line, tokens).len()
}

/// The token index of every heredoc delimiter: the operator token in the
/// attached form (`<<WORD`, `<<-WORD`, `<<"Q"`, `<<""`, `3<<WORD`) and the
/// following token in the bare `<<` / `<<-` form. The io-number prefix shifts
/// the raw start the attached test reads, so `3<<T` is attached on `T` and
/// `3<< T` takes the next word (the unshifted test would read `<T`).
pub(crate) fn delimiter_token_indices(line: &[u8], tokens: &[Token]) -> Vec<usize> {
    operator_indices(line, tokens)
        .into_iter()
        .filter_map(|i| {
            let (_t, start, end, _fq, _mask, _spans) = tokens.get(i)?;
            let (op, _prefix) = span_operator(line, *start, *end)?;
            if attached(line, op, *end) {
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
    operator_indices(line, tokens)
        .into_iter()
        .map(|i| operator_at(line, tokens, i).map(|(op, _extra)| op))
        .collect()
}

/// The operator at `i`: its delimiter and target descriptor, and how many
/// following tokens it consumes (1 for the bare `<<` / `<<-` form, whose
/// delimiter is the next word). The `<<-` marker is inherited by that word, so
/// `<<- -` strips.
pub(super) fn operator_at(
    line: &[u8],
    tokens: &[Token],
    i: usize,
) -> Result<(Operator, usize), Report<ParseError>> {
    let Some((_t, start, end, _fq, _mask, _spans)) = tokens.get(i) else {
        bail!(ParseError::Never);
    };
    let (op, _prefix) = span_operator(line, *start, *end).ok_or(ParseError::Never)?;
    let digits = line.get(*start..op).ok_or(ParseError::Never)?;
    let Some(fd) = io_fd(digits) else {
        let prefix = ShortCStr::from_vec(digits.to_vec()).change_context(ParseError::Never)?;
        bail!(ParseError::InvalidHeredocFd { prefix });
    };
    let (strip, delim) = operator_delim(line, op, *end);
    if !delim.is_empty() {
        // Attached form: `<<WORD`, `<<-WORD`, `<<"Q"`, `<<""`, `3<<WORD`.
        return Ok((Operator::new(delim, strip, fd), 0));
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
    Ok((Operator::new(raw, strip, fd), 1))
}
