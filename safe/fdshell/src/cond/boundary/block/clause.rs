//! The **clause/body position** of a byte inside keyword-block text, fed by the
//! cond-boundary scan's own word walk: the heredoc region rule
//! (`cond/boundary/body.rs`) scans a body region for a top-level `&&` / `||`
//! only where the `<<` that claimed it sits in the block's **clause** (its
//! condition, word list or pattern text); a region claimed by a word of the
//! block's **body** is the body's own data.
//!
//! The position is not answerable from the open-block **pairing**: `then`/`do`
//! score `0` in [`super::word_delta`], so the pairing is the same (one block
//! open) both in `if cat <<X; then` (the `<<` is in the condition) and in
//! `if :; then cat <<X` (it is in the body).
//! The discriminating rows are the clause keywords, i.e. the twin rule
//! `keywords::position::after` runs on the same words, read here as a mode:
//! `then`/`else`/`do` and a `case` arm's `)` open a **body**, `elif` and `in`
//! open a **clause**, an opener pushes a clause, a closer pops its block.
//!
//! Braces push nothing: a `{ …; }` group is counted by `cond/boundary.rs`'s own
//! `group_depth` and never by the block pairing ([`super::word_delta`] excludes
//! brace words), so a group written in a block's clause keeps the clause mode
//! and a group written in its body keeps the body mode.

use crate::keywords::{Place, body_keyword, clause_keyword, first_word_end, list_keyword};
use alloc::vec::Vec;

use super::glue::strip_at;

/// Which half of the innermost open keyword block a byte sits in.
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub(in crate::cond) enum Mode {
    /// The block's clause: its condition, its word list, its pattern text.
    Clause,
    /// The block's body: behind `then`/`else`/`do`, behind a `case` arm's `)`.
    Body,
}

impl Mode {
    /// `true` for [`Mode::Body`]. The production scan's only mode comparison:
    /// the region rule tests the mode with `matches!` and [`Modes::sync`] with
    /// this, so `PartialEq`/`Eq` stay test-only derives.
    fn is_body(self) -> bool {
        matches!(self, Mode::Body)
    }
}

/// The innermost block's mode along the scanned text, carried as the sequence
/// of its changes: the scan feeds it every word start it visits ([`Modes::scan`],
/// the same walk that already scores the words), and the region rule asks one
/// question of it ([`Modes::mode_at`]).
pub(in crate::cond) struct Modes {
    /// One entry per open keyword block, innermost last: that block's mode.
    stack: Vec<Mode>,
    /// `(byte index, mode)` of every change of the innermost mode, in order.
    transitions: Vec<(usize, Mode)>,
}

impl Modes {
    pub(in crate::cond) fn new() -> Self {
        Self {
            stack: Vec::new(),
            transitions: Vec::new(),
        }
    }

    /// Score the word starting at `i`. The scan hands in what it already
    /// computed — the pairing `delta` of the shared scorer and the word's
    /// own [`Place`] — so no keyword is scored twice, the clause gate stays
    /// `after`'s own (`place != Place::Data`), and the walk's heredoc-region,
    /// comment and quote skips are inherited, never duplicated.
    pub(in crate::cond) fn scan(&mut self, line: &[u8], i: usize, place: Place, delta: i32) {
        // A word that starts inside a glued `&&` / `||` pair is the pair's
        // payload, scored at its own bytes by the scorer; the mode stays.
        if strip_at(line, i) != 0 {
            return;
        }
        let tail = line.get(i..).unwrap_or(b"");
        let w = line
            .get(i..first_word_end(tail, i))
            .unwrap_or(b"")
            .trim_ascii();
        if delta > 0 {
            self.stack.push(Mode::Clause);
        } else if delta < 0 {
            self.stack.pop();
        } else if !matches!(place, Place::Data) && body_keyword(w) {
            self.set_top(Mode::Body);
        } else if !matches!(place, Place::Data) && (clause_keyword(w) || list_keyword(w)) {
            self.set_top(Mode::Clause);
        } else if w.ends_with(b")") {
            self.set_top(Mode::Body);
        }
        self.sync(i);
    }

    /// The mode of the byte `at`: the mode of the last change at/before it, and
    /// [`Mode::Clause`] when the walk recorded none — the conservative fallback
    /// that keeps the pre-#304 scan for a site whose block state is unknown (a
    /// run whose window starts past the opener, a synthetic unit-test text).
    pub(in crate::cond) fn mode_at(&self, at: usize) -> Mode {
        self.transitions
            .iter()
            .rev()
            .find(|&&(index, _)| index <= at)
            .map_or(Mode::Clause, |&(_, mode)| mode)
    }

    /// The innermost block's mode; `Clause` with no open block.
    fn top(&self) -> Mode {
        self.stack.last().copied().unwrap_or(Mode::Clause)
    }

    /// Move the innermost block to `mode`. No open block: nothing moves, the
    /// [`Mode::Clause`] fallback keeps such a site scanned.
    fn set_top(&mut self, mode: Mode) {
        if let Some(top) = self.stack.last_mut() {
            *top = mode;
        }
    }

    /// Record the innermost mode at `at`, only when it changed: the list stays
    /// as short as the text's clause/body transitions. A stray closer on an
    /// empty stack clamps at the fallback (a closer of a **foreign** kind never
    /// reaches the pop at all: [`super::word_delta`] scores it `0`, task #309).
    fn sync(&mut self, at: usize) {
        let mode = self.top();
        let last = self
            .transitions
            .last()
            .copied()
            .map_or(Mode::Clause, |(_, m)| m);
        if last.is_body() != mode.is_body() {
            self.transitions.push((at, mode));
        }
    }
}

#[cfg(test)]
mod tests;
