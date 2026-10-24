//! The **recognition table** of the block keywords: which block a scored
//! keyword word opens, and which block a closer belongs to (task #309).
//!
//! `keyword_delta` is this table's sign projection (`Opens` → `1`, `Closes` →
//! `-1`); the sign alone is the **type-blind** pairing that let a `while` body's
//! `fi` close the `while` (LESSONS "One clause rule, three scorers", the #309
//! entry). The kind half is what [`super::pairing::Pairing`] pairs with, so the
//! four scorers of block text share one opener/closer table and cannot drift:
//! a closer closes *its own* block, a closer of another kind written in a body
//! is word data.
//!
//! The rows are `keyword_delta`'s, verbatim: the [`quoted_word`] gate, the
//! `time [-p]` prefix strip, the closer's `&|<>` boundary set (the glued
//! `done>f` form, whose token-side twin is `parse::block_tail::is_closer`), the
//! [`wait_opens_block`] disambiguation of the event-`wait` block from the POSIX
//! builtin, and the group's `{`/`}` predicates from
//! [`crate::segment::group::{opens, closes}`].

use super::{quoted_word, time_prefix_len, wait_opens_block};
use crate::segment::group::{closes, opens};

/// The closer an open block requires: the type half of the opener/closer
/// pairing. `Done` is bash's closer of every loop (`while`/`until`/`for`) and of
/// the event-`wait` block; `Brace` is the `{ …; }` group's, owned by the group
/// scanner's own counter, never by the block-depth pairing.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub(crate) enum Closer {
    Fi,
    Done,
    Esac,
    Brace,
}

/// A scored block keyword word: the block it opens (carrying the closer that
/// block will require) or the closer it is (carrying the block it closes).
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub(crate) enum Keyword {
    Opens(Closer),
    Closes(Closer),
}

/// Check that `word` ends at a token boundary at position `len`. Openers accept
/// whitespace, `;` and the end of the word only; a **closer** also accepts the
/// redirection operator bytes (`&|<>`), so `fi>f` closes the block and leaves
/// the operator to `parse::block_tail`.
fn boundary(word: &[u8], len: usize, extra: &[u8]) -> bool {
    word.get(len)
        .is_none_or(|&b| b.is_ascii_whitespace() || b == b';' || extra.contains(&b))
}

/// The block keyword a scored word is, `None` for a word that is no keyword:
/// a quoted word ([`quoted_word`] — bash reads a quoted keyword as data), a
/// prefix like `ifconfig`/`donec`, the POSIX builtin `wait`, the glued
/// `for((…))` opener (task #396, not a keyword word for the scorer), and every
/// ordinary word. A `time [-p]` word is a *prefix*, not a keyword of its own:
/// the keyword behind it is scored on the rest of the word, which keeps such a
/// statement one segment for the `time` parse hook.
pub(crate) fn keyword_kind(word: &[u8], line: &[u8], word_end: usize) -> Option<Keyword> {
    if quoted_word(line, word_end) {
        return None;
    }
    if let Some(p) = time_prefix_len(word) {
        return keyword_kind(word.get(p..).unwrap_or(b""), line, word_end);
    }
    if word.starts_with(b"case") && boundary(word, 4, b"") {
        return Some(Keyword::Opens(Closer::Esac));
    }
    if word.starts_with(b"esac") && boundary(word, 4, b"&|<>") {
        return Some(Keyword::Closes(Closer::Esac));
    }
    if word.starts_with(b"if") && boundary(word, 2, b"") {
        return Some(Keyword::Opens(Closer::Fi));
    }
    if word.starts_with(b"fi") && boundary(word, 2, b"&|<>") {
        return Some(Keyword::Closes(Closer::Fi));
    }
    if word.starts_with(b"for") && boundary(word, 3, b"") {
        return Some(Keyword::Opens(Closer::Done));
    }
    // The event-`wait` opener and the POSIX builtin, one row: the builtin opens
    // no block, so it is **no keyword** (`keyword_delta` scored it `Some(0)`,
    // which every consumer reads as "no delta").
    if word.starts_with(b"wait") && boundary(word, 4, b"") {
        return wait_opens_block(line, word_end).then_some(Keyword::Opens(Closer::Done));
    }
    if (word.starts_with(b"while") || word.starts_with(b"until")) && boundary(word, 5, b"") {
        return Some(Keyword::Opens(Closer::Done));
    }
    if word.starts_with(b"done") && boundary(word, 4, b"&|<>") {
        return Some(Keyword::Closes(Closer::Done));
    }
    if opens(word, line, word_end) {
        return Some(Keyword::Opens(Closer::Brace));
    }
    if closes(word) {
        return Some(Keyword::Closes(Closer::Brace));
    }
    None
}

#[cfg(test)]
mod tests;
