//! For-list word helpers: classifying a whole-word backtick.

/// A word is a whole-word backtick command substitution when it starts AND
/// ends with a backtick. `$(…)` words are not special here — they go through
/// the argument substitution pipeline.
pub(super) fn is_backtick(bs: &[u8]) -> bool {
    bs.len() >= 2 && bs.first() == Some(&b'`') && bs.last() == Some(&b'`')
}

pub(super) fn strip_delims(bs: &[u8]) -> &[u8] {
    bs.get(1..bs.len() - 1).unwrap_or(b"")
}
