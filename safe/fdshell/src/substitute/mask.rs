//! Quote-mask helpers for the substitution pass.

use alloc::vec::Vec;
use core::cell::Cell;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use crate::error::resolve::ResolveError;

/// Counts the bytes it yields so the caller can see how many input bytes a
/// substitution helper consumed from the shared peekable.
pub(super) struct Counting<'a, I: Iterator> {
    pub(super) inner: I,
    pub(super) consumed: &'a Cell<usize>,
}

impl<'a, I: Iterator> Iterator for Counting<'a, I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<I::Item> {
        let item = self.inner.next();
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

/// After a helper consumed input from the shared peekable, sync `idx` to the
/// consumption count and pad `mask` to the new output length.
pub(super) fn realign(
    idx: &mut usize,
    consumed: &Cell<usize>,
    mask: &mut Vec<bool>,
    before: usize,
    len: usize,
    quoted: bool,
) {
    *idx = consumed.get();
    pad_mask(mask, before, len, quoted);
}
