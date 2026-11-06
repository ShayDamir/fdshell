//! The **pairing** of open blocks with the keyword words of a scan (task
//! #309): the depth a scorer used to keep as an `i32`, now typed.
//!
//! Every scorer of block text pairs an opener with *some* closer; the question
//! this module answers is *which* one. The stack is the sequence of open blocks,
//! innermost last, and [`Pairing::apply`] is the **one** place the pairing
//! score lives, so the four scorers (the cond-boundary block counter, the
//! block-span scan, the parse closing-keyword search, and the `Place`
//! transition's closer row) share it verbatim:
//!
//! * an opener pushes its own closer kind and scores `+1`;
//! * a closer that matches the innermost block pops it and scores `-1`;
//! * a closer of **another kind** scores `0` — it is word data, not the end of
//!   the block, and a body written past it keeps its tail (the hang family of
//!   LESSONS "an argument-position opener plus a mismatched closer in a loop
//!   body ⇒ the body never returns");
//! * a word that is no keyword scores `0`, and with **nothing open** (top level)
//!   a closer has no block to mismatch against, so it scores `-1` and pops
//!   nothing — the stray-closer clamp every top-level scan pins, whose depth
//!   callers already clamp at `0`.
//!
//! The score is what the scorers read, and no caller may move it (task #309).
//! Task #436 adds the [`Decision`] a scan takes on top of it **at its own top
//! level** (an empty stack) — a read of the same stack, never a second table,
//! and never a score change.

use super::Place;
use super::kind::{Closer, Keyword, keyword_kind};
use alloc::vec::Vec;

/// What bash's grammar makes a closer word that **cannot pair** (task #436).
/// [`Pairing::apply`] is the score and stays a score — an unpaired closer scores
/// `0` and the block keeps its tail — but bash parses the whole input before it
/// runs anything, and its grammar has no command that is a lone closing keyword
/// with no open block of its kind, so such a word is a syntax error and nothing
/// on the line runs. [`Pairing::decision`] is that decision, read from the same
/// stack and the same [`keyword_kind`] table: one rule, never a second table.
///
/// Only a word in **command position** with **nothing open** is asked — a
/// closer-shaped argument (`echo fi`, `for i in done`, `case x in done) …`) is
/// word data in bash too, and the place gate of [`super::position`] owns that
/// half.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub(crate) enum Decision {
    /// The word closes the innermost open block — the arm [`Pairing::apply`]
    /// pops, and the pairing's own answer (task #309).
    Paired,
    /// The word closes **no** open block of its kind: its kind mismatches the
    /// innermost block, or nothing is open — the top level, where a closer has
    /// no block to mismatch against (the arm whose score every top-level scan
    /// already clamps at `0`). The scan of task #436 reports only the
    /// empty-stack arm: a mismatch says the *scanner* did not pair the word, not
    /// that bash would refuse it (`comment/depth_delta/word_step`).
    Stray,
}

/// The open blocks of one scan, innermost last. Per-scorer by construction: the
/// span scan pairs braces (its group words are scored), the cond-boundary scan
/// does not (its group arm's own `group_depth` owns them), and each keeps its
/// own stack.
pub(crate) struct Pairing {
    stack: Vec<Closer>,
    /// Task #436's record, read once per scan: the offset of the first word
    /// [`Pairing::observe`] marks stray (see [`Pairing::decision`]). A scorer
    /// that walks text it will never run simply never reads it; the top-level
    /// scan of `comment.rs` hands it to the segment bookkeeping.
    stray: Option<usize>,
}

impl Pairing {
    /// A scan with nothing open: the top level, the `gate_end` start, an arm's
    /// stray-pattern walk.
    pub(crate) fn new() -> Self {
        Self {
            stack: Vec::new(),
            stray: None,
        }
    }

    /// The recorded stray offset of this scan (see [`Pairing::observe`]).
    pub(crate) fn stray(&self) -> Option<usize> {
        self.stray
    }

    /// Read [`Pairing::decision`] for one word **before** it is [`Pairing::apply`]-ed
    /// and keep the first `Stray` (task #436). The gate is the command-position
    /// place — an argument word is never asked — and the **empty** stack, so the
    /// mismatch arm of [`Pairing::apply`] records nothing: it only says the scanner
    /// did not pair the word, and its score-0 answer is load-bearing exactly there
    /// (`tests/cond_blocks.rs::a_closer_shaped_case_pattern_list_word_is_a_word`).
    pub(crate) fn observe(&mut self, word: &[u8], line: &[u8], word_end: usize, place: Place) {
        if matches!(place, Place::Command)
            && self.top().is_none()
            && self.stray.is_none()
            && self.decision(word, line, word_end) == Some(Decision::Stray)
        {
            self.stray = Some(word_end - word.len());
        }
    }

    /// Seed the scan with the block the caller is scanning: a block span scan
    /// starts **inside** its opener, so the opener word itself is never read by
    /// the walk and the caller seeds the closer that block requires.
    pub(crate) fn open(&mut self, closer: Closer) {
        self.stack.push(closer);
    }

    /// The innermost open block, `None` when nothing is open: the gate of the
    /// `Place` transition's closer row (`keywords/position::score`).
    pub(crate) fn top(&self) -> Option<Closer> {
        self.stack.last().copied()
    }

    /// A block is still open — the `depth > 0` test every scorer used to read.
    pub(crate) fn is_open(&self) -> bool {
        !self.stack.is_empty()
    }

    /// The number of open blocks — the **depth** a walk carries. `0` only at the
    /// top level; a caller that seeded the block it is inside (seed depth `1`)
    /// asks "am I back where I started?" rather than "is anything open", because
    /// its own block's seed must stay on the stack for the answer to mean
    /// "this word is at the level the caller walks" (`parse/semi/closing/walk.rs`,
    /// task #418). `Pairing` owns the stack, so the depth is read here and no
    /// caller re-derives it.
    pub(crate) fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Pair one **scored** keyword word with the open blocks: push an opener's
    /// kind, pop only a closer that matches the innermost block. The delta to
    /// add — `0` when the closer mismatches, because the word is then word data.
    ///
    /// A caller that gates the score (by [`super::scores`]) applies this only to
    /// the words that survive its gate; the pairing itself never re-reads the
    /// place, so the *place* and *kind* gates stay orthogonal.
    pub(crate) fn apply(&mut self, word: &[u8], line: &[u8], word_end: usize) -> i32 {
        match keyword_kind(word, line, word_end) {
            Some(Keyword::Opens(closer)) => {
                self.stack.push(closer);
                1
            }
            Some(Keyword::Closes(closer)) => {
                if self.stack.last() == Some(&closer) {
                    self.stack.pop();
                    return -1;
                }
                if self.stack.is_empty() {
                    // Nothing open: no block to mismatch against, so the closer
                    // keeps the top-level score callers clamp at 0.
                    return -1;
                }
                0
            }
            None => 0,
        }
    }

    /// The [`Decision`] for one closer word, read **before** the word is
    /// [`apply`]-ed — the same read [`super::position`] makes when it takes
    /// [`Pairing::top`] as the pairing gate of the closer row. `None` answers a
    /// word that is no keyword (no closer, no decision), an opener (it opens a
    /// block, it closes nothing), and the **brace** closer — the `{ …; }` group
    /// is `segment/group.rs`'s own, its opener rule is tested first and its
    /// scans never consult this decision (task #436's boundary) — so only the two
    /// unpaired arms of [`apply`] **of the keyword closers** are ever asked, and
    /// the answer is `Stray` for both: the kind-mismatch arm and the arm of a
    /// closer seen with nothing open.
    ///
    /// Task #436's rule is asked **only** by the top-level scan, at words the
    /// place gate puts in command position; the nested scorers read the same
    /// pairing for their score and must not report the same word twice.
    pub(crate) fn decision(&self, word: &[u8], line: &[u8], word_end: usize) -> Option<Decision> {
        let kind = match keyword_kind(word, line, word_end) {
            // The brace closer is not asked: bash's grammar makes a lone `}` a
            // command word unless a brace group is *open*, and the scans that
            // know a group is open (`segment/group*`, `parse/function_block*`)
            // are the ones that own it (the boundary above).
            Some(Keyword::Closes(Closer::Brace)) => return None,
            Some(Keyword::Closes(kind)) => kind,
            _ => return None,
        };
        if self.stack.last() == Some(&kind) {
            return Some(Decision::Paired);
        }
        Some(Decision::Stray)
    }
}

#[cfg(test)]
mod tests;
