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
//! The rows are `keyword_delta`'s, verbatim: the [`keyword_is_quoted`] gate (it
//! measures the keyword's own bytes — [`keyword_len`] says how long they are — where
//! [`crate::keywords::quoted`] locates them in the line), the `time [-p]` prefix
//! strip, the **one word-break set**
//! ([`crate::bytes::word_break`], task #396) shared with the
//! tokenizer's keyword split, the [`wait_opens_block`] disambiguation of the
//! event-`wait` block from the POSIX builtin, and the group's `{`/`}` predicates
//! from [`crate::segment::group::{opens, closes}`]. The glued `done>f` closer
//! (whose token-side twin is `parse::block_tail::is_closer`) and the glued
//! `for((…))` opener (whose token-side twin is
//! `parse::token::step::keyword`) are answered by the one boundary rule. The
//! rows that answer *no keyword* are load-bearing: the POSIX builtin `wait`,
//! and the arithmetic `for` whose region has a group opener behind it (task
//! #394, [`arith_for`]) — there the brace pair nets zero, so the word
//! contributes no pairing frame and the group branch takes the whole construct.

pub(crate) mod arith_for;

use super::{keyword_is_quoted, time_prefix_len, wait_opens_block};
use crate::bytes::word_break;
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

/// Check that `word` ends at a token boundary at position `len`: the one word-break
/// set ([`crate::bytes::word_break`]) — whitespace, `;`, and the metacharacters
/// `|&<>()` — or the end of the word, so one rule serves openers and closers and
/// agrees with the tokenizer's keyword split ([`crate::keywords::reserved`]).
/// `fi>f` closes the block and leaves the operator to `parse::block_tail`;
/// `for((…))`'s `(` ends the keyword word (task #396).
fn boundary(word: &[u8], len: usize) -> bool {
    word.get(len).is_none_or(|&b| word_break(b))
}

/// The length of the keyword at the word's start — the run a row matches literally
/// (`done` in `done>f"f"`), i.e. the bytes up to the byte that breaks the word, from
/// the **one word-break set** ([`crate::bytes::word_break`]). For a word that is
/// nothing but its keyword the run is the whole word, and the gate measures the whole
/// word's own bytes (task #413).
fn keyword_len(word: &[u8]) -> usize {
    word.iter()
        .position(|&b| word_break(b))
        .unwrap_or(word.len())
}

/// The block keyword a scored word is, `None` for a word that is no keyword:
/// a quoted word ([`keyword_is_quoted`] — bash reads a quoted keyword as data), a
/// prefix like `ifconfig`/`donec`, the POSIX builtin `wait`, and every
/// ordinary word. A word glued to a metacharacter — the `for((…))` head
/// (task #396) or the `done>f` closer — **is** scored, on the one boundary
/// rule, and the operand behind the glue may be **quoted** (`done>f"f"` is
/// bash's closer with bash's redirection, task #413: the quotes belong to the
/// operand, never to the keyword). A `time [-p]` word is a *prefix*, not a
/// keyword of its own: the keyword behind it is scored on the rest of the word,
/// which keeps such a statement one segment for the `time` parse hook.
pub(crate) fn keyword_kind(word: &[u8], line: &[u8], word_end: usize) -> Option<Keyword> {
    if keyword_is_quoted(word, line, word_end, keyword_len(word)) {
        return None;
    }
    if let Some(p) = time_prefix_len(word) {
        return keyword_kind(word.get(p..).unwrap_or(b""), line, word_end);
    }
    if word.starts_with(b"case") && boundary(word, 4) {
        return Some(Keyword::Opens(Closer::Esac));
    }
    if word.starts_with(b"esac") && boundary(word, 4) {
        return Some(Keyword::Closes(Closer::Esac));
    }
    if word.starts_with(b"if") && boundary(word, 2) {
        return Some(Keyword::Opens(Closer::Fi));
    }
    if word.starts_with(b"fi") && boundary(word, 2) {
        return Some(Keyword::Closes(Closer::Fi));
    }
    if word.starts_with(b"for") && boundary(word, 3) {
        // The arithmetic `for` whose region has a group opener behind it is
        // **no keyword** (task #394): the brace pair nets zero in every scorer, so
        // the word contributes no pairing frame and the group branch takes the
        // whole construct. The run gate is what opens its block.
        if arith_for::opens_a_group(line, word_end) {
            return None;
        }
        return Some(Keyword::Opens(Closer::Done));
    }
    // The event-`wait` opener and the POSIX builtin, one row: the builtin opens
    // no block, so it is **no keyword** (`keyword_delta` scored it `Some(0)`,
    // which every consumer reads as "no delta").
    if word.starts_with(b"wait") && boundary(word, 4) {
        return wait_opens_block(line, word_end).then_some(Keyword::Opens(Closer::Done));
    }
    if (word.starts_with(b"while") || word.starts_with(b"until")) && boundary(word, 5) {
        return Some(Keyword::Opens(Closer::Done));
    }
    if word.starts_with(b"done") && boundary(word, 4) {
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
