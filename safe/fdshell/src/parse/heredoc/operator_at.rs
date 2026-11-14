// mutants: -C --lib -C --test -C heredoc -- -E 'test(/^parse::heredoc::/) | binary(heredoc)'

//! What **operator** a `<<` token is — its delimiter, strip flag and target
//! descriptor. Which tokens are operators at all, and which of them are
//! delimiters, stays in `operator`.

use super::super::Token;
use super::operator::operator_indices;
use crate::error::parse::ParseError;
use crate::scan::heredoc::{Operator, invalid_delimiter, io_fd, operator_delim, span_operator};
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

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
    let Some(Token { start, end, .. }) = tokens.get(i) else {
        bail!(ParseError::Never);
    };
    let (op, _prefix) = span_operator(line, *start, *end).ok_or(ParseError::Never)?;
    let digits = line.get(*start..op).ok_or(ParseError::Never)?;
    let Some(fd) = io_fd(digits) else {
        let prefix = ShortCStr::from_vec(digits.to_vec()).change_context(ParseError::Never)?;
        bail!(ParseError::InvalidRedirFdWord { prefix });
    };
    let (strip, delim) = operator_delim(line, op, *end);
    if !delim.is_empty() {
        // Attached form: `<<WORD`, `<<-WORD`, `<<"Q"`, `<<""`, `3<<WORD`.
        return Ok((Operator::new(delim, strip, fd), 0));
    }
    // Bare `<<` (or `<<-` with a separate word): the next token is the
    // delimiter word; the marker is inherited, so `<<- -` strips tabs. The
    // guard's empty arm is gated on the **raw span**: a quoted empty pair
    // (`<< ""`, `<< ''`) is the word that delimits on the empty line, an
    // empty span is the missing operand.
    let Some(next) = tokens.get(i + 1) else {
        bail!(ParseError::InvalidRedirect);
    };
    let Token {
        word,
        start,
        end,
        mask,
        ..
    } = next;
    if invalid_delimiter(word, mask, word.is_shorter_than_span(*start, *end)) {
        bail!(ParseError::InvalidRedirect);
    }
    let raw = line.get(*start..*end).ok_or(ParseError::Never)?;
    Ok((Operator::new(raw, strip, fd), 1))
}
