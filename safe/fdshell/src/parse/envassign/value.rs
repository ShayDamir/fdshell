//! The value half of a `NAME=value` word: the parsed `Assign`, and the one
//! place its mask and spans are sliced at the `=` (tasks #217, #321).

use crate::mask::MaskBit;
use alloc::vec::Vec;
use sys::ShortCStr;

/// One raw `NAME=value` word: the name, the raw (unexpanded) value word, and
/// the value's per-byte quote mask — the token's word mask sliced at the `=`.
/// The mask travels with the value so every substitution site expands the value
/// as its own quoting says (task #217), never with an empty mask. `spans` is
/// the parallel zero-byte quote-span list, sliced at the same offset (task #321).
#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub struct Assign {
    pub name: ShortCStr,
    pub value: ShortCStr,
    pub mask: Vec<MaskBit>,
    pub spans: Vec<usize>,
}

/// The value's mask: the word mask sliced at the `=` (offset `name.len() + 1`).
/// This is the **only** place that offset exists, so the three assignment
/// surfaces cannot drift apart (LESSONS: slice the word, its mask and its
/// quoted flags with the same offset). A mask shorter than the value degrades
/// to unquoted, the same padding `substitute/arg.rs` applies.
pub fn value_mask(word_mask: &[MaskBit], name_len: usize) -> Vec<MaskBit> {
    word_mask.get(name_len + 1..).unwrap_or_default().to_vec()
}

/// The value's zero-byte quote spans (task #321): sliced at the same
/// `name.len() + 1` offset as [`value_mask`] and shifted down to value indices,
/// so the tilde gate reads the value's own coordinates.
pub fn value_spans(word_spans: &[usize], name_len: usize) -> Vec<usize> {
    let off = name_len + 1;
    word_spans
        .iter()
        .filter(|&&i| i >= off)
        .map(|&i| i - off)
        .collect()
}
