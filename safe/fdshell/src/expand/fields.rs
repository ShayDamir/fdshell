//! The whole-word backtick classifier — the vocabulary of the unquoted
//! pairing rule the tokenizer implements (task #216 deleted the for-list arm
//! that used it; the for-list shape is pinned by `tests/loops.rs`).

/// A word is a whole-word backtick command substitution when it starts AND
/// ends with a backtick. `$(…)` words are not special here — they go through
/// the argument substitution pipeline.
pub(super) fn is_backtick(bs: &[u8]) -> bool {
    bs.len() >= 2 && bs.first() == Some(&b'`') && bs.last() == Some(&b'`')
}

/// The body of a whole-word backtick: the word without its delimiters.
pub(super) fn strip_delims(bs: &[u8]) -> &[u8] {
    bs.get(1..bs.len() - 1).unwrap_or(b"")
}
