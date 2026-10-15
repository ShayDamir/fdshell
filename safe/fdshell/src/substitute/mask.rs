//! Quote-mask helpers for the substitution pass.

use alloc::vec::Vec;
use core::cell::Cell;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use crate::error::resolve::ResolveError;

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
    mask: &mut Vec<bool>,
    b: u8,
    quoted: bool,
) -> Result<(), Report<ResolveError>> {
    out.push_byte(b).change_context(ResolveError::NulByte)?;
    mask.push(quoted);
    Ok(())
}

/// Append `text` to `out`, marking every byte with `quoted` in `mask`.
pub(super) fn push_expanded(
    out: &mut ShortCStr,
    mask: &mut Vec<bool>,
    text: &[u8],
    quoted: bool,
) -> Result<(), Report<ResolveError>> {
    let before = out.len();
    out.push_checked(text)
        .change_context(ResolveError::NulByte)?;
    pad_mask(mask, before, out.len(), quoted);
    Ok(())
}

/// Extend `mask` with `quoted` bits until it is as long as `len`.
fn pad_mask(mask: &mut Vec<bool>, before: usize, len: usize, quoted: bool) {
    for _ in before..len {
        mask.push(quoted);
    }
}

/// Bytes a name arm may consume from `from`: the length of the run of mask
/// bits equal to the bit at `from`, i.e. the quote region that byte belongs
/// to. A name cannot cross out of the region its trigger byte sits in,
/// because the tokenizer's quote regions are what end a name (bash expands
/// `"$v"bc` as the parameter `v` plus the literal `bc`). A missing bit (an
/// empty mask) carries no information, so the run is unbounded and the
/// collection is mask-independent.
pub(super) fn region_len(mask: &[bool], from: usize) -> usize {
    let Some(&quoted) = mask.get(from) else {
        return usize::MAX;
    };
    mask.get(from..)
        .unwrap_or(&[])
        .iter()
        .take_while(|&bit| *bit == quoted)
        .count()
}

/// After a helper consumed input from the shared peekable, sync `idx` to the
/// consumption count and pad `mask` to the output's new length.
pub(super) fn realign(
    idx: &mut usize,
    peek: &Counting<'_>,
    mask: &mut Vec<bool>,
    before: usize,
    out: &ShortCStr,
    quoted: bool,
) {
    *idx = peek.consumed();
    pad_mask(mask, before, out.len(), quoted);
}
