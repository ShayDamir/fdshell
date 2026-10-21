//! Quote-mask helpers for the substitution pass.

use alloc::vec::Vec;
use core::cell::Cell;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;

/// Counts the bytes it **hands out**; a peeked byte is counted only when it is
/// later handed out. `consumed` is therefore exactly the index of the next
/// unread byte, which is what the loop's `idx` and the word's quote mask are
/// indexed by. A plain `Peekable` counts a peeked byte at peek time, so every
/// helper that peeks ahead left `idx` one byte ahead of the mask (task #223:
/// the byte after a quoted expansion read its neighbour's mask bit).
/// The stream type is fixed to the word's bytes, so the helpers take a plain
/// `&mut Counting` instead of a `Peekable<impl Iterator>`.
pub(super) struct Counting<'a> {
    inner: core::iter::Copied<core::slice::Iter<'a, u8>>,
    consumed: Cell<usize>,
    peeked: Option<u8>,
}

impl<'a> Counting<'a> {
    /// A stream over one word's bytes, counting into `consumed`.
    pub(super) fn new(inner: core::iter::Copied<core::slice::Iter<'a, u8>>) -> Self {
        Self {
            inner,
            consumed: Cell::new(0),
            peeked: None,
        }
    }

    /// Bytes handed out so far: the index of the next unread byte.
    pub(super) fn consumed(&self) -> usize {
        self.consumed.get()
    }

    /// The next byte without consuming it; `None` is the end of the word.
    pub(super) fn peek(&mut self) -> Option<&u8> {
        if self.peeked.is_none() {
            self.peeked = self.inner.next();
        }
        self.peeked.as_ref()
    }
}

impl Iterator for Counting<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        let item = self.peeked.take().or_else(|| self.inner.next());
        if item.is_some() {
            self.consumed.set(self.consumed.get() + 1);
        }
        item
    }
}

pub(super) fn push_byte(
    out: &mut ShortCStr,
    mask: &mut Vec<MaskBit>,
    b: u8,
    quoted: bool,
) -> Result<(), Report<ResolveError>> {
    out.push_byte(b).change_context(ResolveError::NulByte)?;
    mask.push(MaskBit::from_quoted(quoted));
    Ok(())
}

/// Append `text` to `out`, marking every byte with `quoted` in `mask`.
pub(super) fn push_expanded(
    out: &mut ShortCStr,
    mask: &mut Vec<MaskBit>,
    text: &[u8],
    quoted: bool,
) -> Result<(), Report<ResolveError>> {
    let before = out.len();
    out.push_checked(text)
        .change_context(ResolveError::NulByte)?;
    pad_mask(mask, before, out.len(), quoted);
    Ok(())
}

/// Extend `mask` with `quoted` bits until it is as long as `len`. Expansion
/// output has no quote regions of its own, so every bit is a plain
/// quoted/unquoted one: a derived mask never claims a region start.
fn pad_mask(mask: &mut Vec<MaskBit>, before: usize, len: usize, quoted: bool) {
    for _ in before..len {
        mask.push(MaskBit::from_quoted(quoted));
    }
}

/// After a helper consumed input from the shared peekable, sync `idx` to the
/// consumption count and pad `mask` to the output's new length.
pub(super) fn realign(
    idx: &mut usize,
    peek: &Counting<'_>,
    mask: &mut Vec<MaskBit>,
    before: usize,
    out: &ShortCStr,
    quoted: bool,
) {
    *idx = peek.consumed();
    pad_mask(mask, before, out.len(), quoted);
}
