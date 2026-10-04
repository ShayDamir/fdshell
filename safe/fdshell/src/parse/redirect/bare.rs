//! Bare redirect operator tokens: `>`, `>>`, `<`, `<>` (with an optional
//! numeric fd prefix) as standalone words. Each takes the next token as its
//! path operand — the token-level counterpart of the bare `<<`/`<<<` forms.

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use error_stack::{Report, bail};
use sys::ShortCStr;

/// The suffix of `s` from its first `>`/`<` byte.
pub(super) fn operator_suffix(s: &ShortCStr) -> Option<ShortCStr> {
    let bytes = s.as_bytes().ok()?;
    let pos = bytes.iter().position(|&b| b == b'>' || b == b'<')?;
    s.get(pos..)
}

/// Whether the token is a bare operator: its suffix from the first `>`/`<`
/// byte is exactly `>`, `>>`, `<`, or `<>` (an optional all-digit fd prefix
/// precedes it). `>>` and `<>` are bare even though a byte follows the first
/// operator byte, so "bare" is a suffix test, not an emptiness test.
pub(super) fn is_bare(s: &ShortCStr) -> bool {
    let Some(op) = operator_suffix(s) else {
        return false;
    };
    op.eq_bytes(b">") || op.eq_bytes(b">>") || op.eq_bytes(b"<") || op.eq_bytes(b"<>")
}

/// The bare operator at `i`: its operand is the next token. A non-numeric
/// fd prefix is not a redirect — the token stays an argument.
pub(super) fn parse_bare(
    tokens: &[Token],
    i: usize,
) -> Result<Option<RedirectDef>, Report<ParseError>> {
    let Some((t, _start, _end, _fq, _mask)) = tokens.get(i) else {
        return Ok(None);
    };
    let Some(op) = operator_suffix(t) else {
        return Ok(None);
    };
    let prefix = t.get(..t.len() - op.len()).ok_or(ParseError::Never)?;
    let dir = if op.eq_bytes(b"<") || op.eq_bytes(b"<>") {
        b'<'
    } else {
        b'>'
    };
    let Some(export_to) = super::parse_fd(&prefix, dir) else {
        return Ok(None);
    };
    let Some((operand, _os, _oe, _ofq, omask)) = tokens.get(i + 1) else {
        bail!(ParseError::InvalidRedirect);
    };
    if invalid_operand(operand) {
        bail!(ParseError::InvalidRedirect);
    }
    if let Some(n) = super::super::fd_path::fd_path_target(operand) {
        return Ok(Some(RedirectDef::dup(export_to, n)));
    }
    Ok(Some(RedirectDef {
        export_to,
        direction: direction(&op),
        source: RedirectSource::path(operand.clone(), omask.clone()),
    }))
}

/// The direction of a bare operator suffix: `>` writes, `>>` appends, `<`
/// reads, `<>` reads and writes.
fn direction(op: &ShortCStr) -> RedirectDirection {
    if op.eq_bytes(b">>") {
        RedirectDirection::Append
    } else if op.eq_bytes(b"<") {
        RedirectDirection::Read
    } else if op.eq_bytes(b">") {
        RedirectDirection::Write
    } else {
        RedirectDirection::Rw
    }
}

/// A bare operator's operand must be a plain word: the same rule as the bare
/// `<<` delimiter (`scan::heredoc::op::invalid_delimiter`) — not empty, not a
/// separator, and not another operator.
fn invalid_operand(word: &ShortCStr) -> bool {
    word.is_empty()
        || word.starts_with(b"<")
        || word.starts_with(b">")
        || word.starts_with(b"&")
        || word.starts_with(b"%")
        || word.eq_bytes(b";")
        || word.eq_bytes(b"|")
}
