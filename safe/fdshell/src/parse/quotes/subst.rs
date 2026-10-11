//! The `$( … )`, `${ … }` and `` ` … ` `` region readers used by the
//! double-quote scanner (task #130).
//!
//! A nested region is scanned as one unit: its bytes go into the word with
//! mask bit `true` (the region is quoted material), `pos` advances by every raw
//! byte it consumed, and the enclosing quote stays open across it — an inner
//! `"` never closes the outer quoted region.

use super::super::token::State;
use crate::error::parse::ParseError;
use crate::region_scan::{scan_backtick, scan_brace};
use error_stack::{Report, ResultExt};

/// Push one quoted word byte (a region delimiter) with its mask bit.
fn push_byte(st: &mut State, b: u8) -> Result<(), Report<ParseError>> {
    st.cur
        .push_byte(b)
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    st.mask.push(true);
    Ok(())
}

/// Push a region body: one mask bit per byte pushed.
fn push_body(st: &mut State, body: &[u8]) -> Result<(), Report<ParseError>> {
    st.cur
        .push_checked(body)
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    st.mask.extend(core::iter::repeat_n(true, body.len()));
    Ok(())
}

/// Consume the region's raw bytes from the stream: the scanner worked on the
/// slice, so the stream is advanced by the bytes after the caller's byte
/// (`start` is that byte's index, and the token loop already consumed it).
fn take(bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>, consumed: usize) {
    for _ in 1..consumed {
        bytes.next();
    }
}

/// Read a `$( … )` region inside double quotes. Its body is kept verbatim
/// (quote bytes included) because `cmd_subst` re-parses it with its own
/// quote-aware scan, and it must close, exactly like the unquoted path.
pub(super) fn dollar_paren(
    st: &mut State,
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    line: &[u8],
) -> Result<(), Report<ParseError>> {
    st.read_dollar_paren(bytes, line, true)
}

/// Read a `${ … }` region inside double quotes. Its quotes are dropped: the
/// substitution pass takes the braced content verbatim, so a kept `"` would
/// print literally (`${u:-"z w"}` → `z w`, like bash). An unterminated region
/// consumes what it can and returns, leaving the enclosing quote open, so the
/// parse reports the real unbalanced quote.
pub(super) fn brace(
    st: &mut State,
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    line: &[u8],
) -> Result<(), Report<ParseError>> {
    let (body, consumed, closed) = scan_brace(line, st.pos - 1);
    take(bytes, consumed);
    // `consumed` starts at the `$`, which the token loop already counted.
    st.pos += consumed.saturating_sub(1);
    push_byte(st, b'$')?;
    push_byte(st, b'{')?;
    push_body(st, &body)?;
    if closed {
        push_byte(st, b'}')?;
    }
    Ok(())
}

/// Read a `` ` … ` `` region inside double quotes, terminated by the first
/// unescaped, unquoted backtick; its quotes are dropped like the `${ … }`
/// body. fdshell never expands backticks (task #216): the text is literal.
pub(super) fn backtick(
    st: &mut State,
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    line: &[u8],
) -> Result<(), Report<ParseError>> {
    let (body, consumed, closed) = scan_backtick(line, st.pos - 1);
    take(bytes, consumed);
    // `consumed` starts at the backtick, which the token loop already counted.
    st.pos += consumed.saturating_sub(1);
    push_byte(st, b'`')?;
    push_body(st, &body)?;
    if closed {
        push_byte(st, b'`')?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
