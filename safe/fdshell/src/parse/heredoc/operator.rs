//! Token-level `<<` operator detection for heredoc parsing — the token
//! counterpart of the byte-level rule in `scan::heredoc::operator_delims`.

use super::super::Token;
use crate::error::parse::ParseError;
use crate::parse::word_quoted::word_quoted;
use crate::scan::heredoc::{
    Operator, attached, invalid_delimiter, io_fd, operator_delim, span_operator,
};
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

/// Whether the token at `i` is a heredoc operator: an unquoted `<<` word
/// (not `<<<`) with the io-number form `N<<word` included, at **any** word
/// position — word 0 included, since POSIX #2.9 makes a redirections-only
/// statement (`simple_command = [NAME=value…] REDIRECT… [word…]`) a command of
/// its own, and a pipeline stage is such a statement in its own right
/// (`cat "a>" | <<EOF` feeds the body to an empty stage, task #345). Nothing
/// about the **predecessor** is consulted: task #261 required a preceding token
/// that is not a pipeline `|`, and task #345 removed both halves of that rule —
/// the word-0 arm and the pipe arm (the *pipeline split* rule,
/// `parse::redirect::pipe_token_span`, is untouched and owns that shape).
/// The shape test reads the token's **raw span** and the shared
/// `span_operator`, never the token text: quote removal makes `3"<<"T` read
/// exactly like `3<<T`, and a token-text rule for a raw-byte structure is the
/// byte/token count mismatch (LESSONS). The byte-level twin of this predicate
/// is `scan::heredoc::ops::opens_word`, whose word-break arms are the
/// tokenizer's separators plus the pipeline `|` (the ONE rule
/// `parse::redirect::pipeline_pipe`).
pub(crate) fn is_operator(line: &[u8], tokens: &[Token], i: usize) -> bool {
    let Some((_t, start, end, fq, _mask, _spans)) = tokens.get(i) else {
        return false;
    };
    !*fq && span_operator(line, *start, *end).is_some()
}

/// The token index of every heredoc operator, word 0 included.
fn operator_indices(line: &[u8], tokens: &[Token]) -> Vec<usize> {
    (0..tokens.len())
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
    // delimiter word; the marker is inherited, so `<<- -` strips tabs. The
    // guard's empty arm is gated on the **raw span**: a quoted empty pair
    // (`<< ""`, `<< ''`) is the word that delimits on the empty line, an
    // empty span is the missing operand.
    let Some(next) = tokens.get(i + 1) else {
        bail!(ParseError::InvalidRedirect);
    };
    let (word, start, end, _fq, mask, _spans) = next;
    if invalid_delimiter(word, mask, word_quoted(word, *start, *end)) {
        bail!(ParseError::InvalidRedirect);
    }
    let raw = line.get(*start..*end).ok_or(ParseError::Never)?;
    Ok((Operator::new(raw, strip, fd), 1))
}
