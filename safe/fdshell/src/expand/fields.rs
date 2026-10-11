//! For-list word helpers: classifying a whole-word backtick and splitting a
//! substitution's output into shell words.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use crate::error::resolve::ResolveError;

/// A word is a whole-word backtick command substitution when it starts AND
/// ends with a backtick. `$(…)` words are not special here — they go through
/// the argument substitution pipeline.
pub(super) fn is_backtick(bs: &[u8]) -> bool {
    bs.len() >= 2 && bs.first() == Some(&b'`') && bs.last() == Some(&b'`')
}

pub(super) fn strip_delims(bs: &[u8]) -> &[u8] {
    bs.get(1..bs.len() - 1).unwrap_or(b"")
}

pub(super) fn split_whitespace(data: &[u8]) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    let mut words = Vec::new();
    let mut cur = ShortCStr::new();
    for &b in data {
        if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
            if !cur.is_empty() {
                words.push(core::mem::take(&mut cur));
            }
        } else {
            cur.push_byte(b).change_context(ResolveError::NulByte)?;
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    Ok(words)
}
