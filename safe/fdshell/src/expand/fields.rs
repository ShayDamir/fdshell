//! For-list word helpers: classifying a word's bytes (whole-word `$(())`
//! arithmetic, `$(…)`/backtick command substitution) and splitting a
//! substitution's output into shell words.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use crate::error::resolve::ResolveError;

pub(super) fn is_arith(bs: &[u8]) -> bool {
    bs.len() >= 4 && bs.starts_with(b"$((") && bs.ends_with(b"))")
}

pub(super) fn is_cmd_subst(bs: &[u8]) -> bool {
    (bs.first() == Some(&b'`') && bs.last() == Some(&b'`') && bs.len() >= 2)
        || (bs.len() >= 3 && bs.starts_with(b"$(") && bs.last() == Some(&b')'))
}

pub(super) fn strip_delims(bs: &[u8]) -> &[u8] {
    if bs.starts_with(b"$(") {
        bs.get(2..bs.len() - 1).unwrap_or(b"")
    } else {
        bs.get(1..bs.len() - 1).unwrap_or(b"")
    }
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
