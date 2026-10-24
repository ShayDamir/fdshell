//! The opening-keyword length, shared by the run-start block scan
//! ([`super::keyword::keyword_block`]'s scan seed) and the mid-run cond-part skip
//! ([`super::midrun::skip`]'s span start), so the two spans of the same keyword
//! are seeded at the same byte.

use crate::keywords::Place;

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

/// The length of the `for` keyword: `for` is the one opener that owns a **word
/// list** the span scan must read as words (task #306), so a caller that has
/// measured [`opener_len`] picks the scan's seed place from the length it
/// already has — no second keyword table, and no literal for `cargo mutants` to
/// move (`FOR_LEN` is derived from the keyword's own bytes).
pub(super) const FOR_LEN: usize = b"for".len();

/// The [`Place`] a block span scan starts its **first run** in (task #306): a
/// `for` opener owns a word list, whose closer-shaped words must not end the
/// span (`for i in done; do …` iterates over the word `done`), so the scan of a
/// `for` block is seeded with [`Place::Words`]. Every other opener starts at a
/// command position, exactly as the scan did before: `case`'s **pattern** list
/// is the opposite place (`esac` terminates it) and is task **#310**'s half,
/// and the closer-shaped word of an `if`/`while`/`until` **condition** is an
/// argument, never a list word.
pub(super) fn list_seed(kw_len: usize) -> Place {
    if kw_len == FOR_LEN {
        Place::Words
    } else {
        Place::Command
    }
}
