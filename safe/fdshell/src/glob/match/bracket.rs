//! Bracket expressions `[...]`: span validity and membership.
//!
//! After `[`, an unquoted `!` negates; a `]` as the first byte is a literal
//! member; the next unquoted `]` closes. Quoted bytes and unquoted `\X` pairs
//! are literal members. The `[:class:]` and `a-z` arms a member may take are
//! in `member::arm`, the scan's cursor state in `member::Members`.

mod class;
mod member;
mod span;

use crate::mask::MaskBit;
pub(in crate::glob) use span::{quoted, span};

/// Whether `b` is a member of the (valid) bracket expression at `at_open`.
pub(super) fn contains(bytes: &[u8], mask: &[MaskBit], at_open: usize, b: u8) -> bool {
    member::Members::scan(bytes, mask, at_open, b)
}
