//! The **pairing** of open blocks with the keyword words of a scan (task
//! #309): the depth a scorer used to keep as an `i32`, now typed.
//!
//! Every scorer of block text pairs an opener with *some* closer; the question
//! this module answers is *which* one. The stack is the sequence of open blocks,
//! innermost last, and [`Pairing::apply`] is the **one** place the pairing
//! decision lives, so the four scorers (the cond-boundary block counter, the
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

use super::kind::{Closer, Keyword, keyword_kind};
use alloc::vec::Vec;

/// The open blocks of one scan, innermost last. Per-scorer by construction: the
/// span scan pairs braces (its group words are scored), the cond-boundary scan
/// does not (its group arm's own `group_depth` owns them), and each keeps its
/// own stack.
pub(crate) struct Pairing {
    stack: Vec<Closer>,
}

impl Pairing {
    /// A scan with nothing open: the top level, the `gate_end` start, an arm's
    /// stray-pattern walk.
    pub(crate) fn new() -> Self {
        Self { stack: Vec::new() }
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
}

#[cfg(test)]
mod tests;
