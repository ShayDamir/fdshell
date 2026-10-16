//! The opening-keyword length, shared by the run-start block scan
//! ([`super::keyword::keyword_block`]'s scan seed) and the mid-run cond-part skip
//! ([`super::midrun::skip`]'s span start), so the two spans of the same keyword
//! are seeded at the same byte.

/// The length of the block-opening keyword at the start of `word`:
/// `case`/`wait` (4), `if` (2), `for` (3), `while`/`until` (5), when the keyword
/// ends at a word boundary (whitespace, `;`, or the end of the word — the
/// boundary rule `keywords::keyword_delta` scores with, so the measured length
/// and the scored keyword cannot drift).
///
/// `None` for every other word: the group brace (the `{ …; }` group is owned by
/// `segment/group.rs`, whose opener rule the scanner tests first), a closing
/// keyword, a quoted keyword word, and a keyword prefix like `whileish` — the
/// word behind a `time [-p]` prefix run is scored on the stripped text, the
/// same rule [`crate::keywords::keyword_delta`] applies.
pub(super) fn opener_len(word: &[u8]) -> Option<usize> {
    let len = if word.starts_with(b"case") || word.starts_with(b"wait") {
        4
    } else if word.starts_with(b"if") {
        2
    } else if word.starts_with(b"for") {
        3
    } else if word.starts_with(b"while") || word.starts_with(b"until") {
        5
    } else {
        return None;
    };
    word.get(len)
        .is_none_or(|&b| b.is_ascii_whitespace() || b == b';')
        .then_some(len)
}
