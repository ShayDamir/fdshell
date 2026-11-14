// mutants: -C --lib -C --test -C semi_semi -C --test -C heredoc -C --test -C async -- -E 'test(/^segment::/) | binary(semi_semi) | binary(heredoc) | binary(async)'

//! The marks the scan leaves on the book when a separator ends a run: the
//! segments the preflight owns — the line-leading `;` (task #487), the stray
//! `;;` pair (task #423) and the closing keyword that closes no block
//! (task #436). The loop keeps its own run bookkeeping.

use crate::keywords::{Decision, Pairing, keyword_len};
use crate::scan::Boundary;
use crate::segment::Segment;
use crate::segment::book::Book;

impl<'a> Book<'a> {
    /// Task #487's family, marked next to the `;;` mark because the
    /// splitter erases what it decides on: a real-`;` boundary at the
    /// **head of the scan unit** has an empty run in front of it — the
    /// separator opened the statement (`if !part.is_empty()` erases the
    /// run, so no other layer sees it) — and that is bash's syntax error
    /// (rc 2, nothing after it runs; a complete earlier line runs first),
    /// so the mark rides on the preflight (`script.rs`, whose arm is
    /// first). Top-level scan units are physical lines, so the unit head
    /// **is** the physical-line-start view the rule needs (`x\n; y` and
    /// the rest of the family report through the per-line mechanism).
    /// Byte-gated on the real `;` (the newline's `;` and end of input
    /// keep their measured leniency — `echo a` + blank line + `b` is
    /// parity), mid-scan empties stay silent (`x ; y`, `x ; ; y`, the
    /// `;;` pair's own mark, a `;` behind a block closer), and the
    /// `!in_block` gate is load-bearing for task #300's pinned parse
    /// trim leniency (`{ ; x; }`, `then\n; x`, `do ; x` — "stay as they
    /// are").
    pub(super) fn mark_leading_separator(&mut self, line: &[u8], kind: &Boundary, in_block: bool) {
        if kind == &Boundary::Separator
            && !in_block
            && self.i == 0
            && line.get(self.i) == Some(&b';')
        {
            self.segments.push(Segment::LeadingSeparator);
        }
    }

    /// A `;;` pair here is the stray of task #423: the run was not
    /// claimed as a block, so this separator is not a `case` clause's
    /// terminator, and the mark is the preflight's business (bash runs
    /// nothing). The run path is the only place it can arise.
    pub(super) fn mark_semi_semi(&mut self, kind: &Boundary) {
        if kind == &Boundary::SemiSemi {
            self.segments.push(Segment::UnexpectedSemiSemi);
        }
    }

    /// Task #436's decision, on the run's **first** word — the top level's
    /// command position, with this scan's stack empty: a closing keyword
    /// word here closes no open block of its kind. A run that was claimed
    /// as a block reports through its own span walk above. The word's own
    /// offset is computed once: the ask and the report read the same number.
    pub(super) fn mark_stray_closer(
        &mut self,
        line: &[u8],
        word: &[u8],
        word_off: usize,
        in_block: bool,
    ) {
        if !in_block && is_stray_closer(line, word, word_off) {
            self.segments.push(Segment::StrayCloser { off: word_off });
        }
    }
}

/// The [`Decision`] of task #436 for the run's first word — the top level's
/// command position, so [`Pairing`] is empty and any closing keyword word closes
/// no open block of its kind. The word is the run's first `keyword_len` bytes, so
/// the **one** recognition table answers here exactly as it does for the scorers
/// (the quote gate, the `time [-p]` strip, the one word-break rule); `off` is the
/// word's own offset in `line`, which is where the report draws the caret. An
/// ordinary word (and an empty run) is no closer, so the answer is `None`.
pub(super) fn is_stray_closer(line: &[u8], word: &[u8], off: usize) -> bool {
    Pairing::new().decision(word, line, off + keyword_len(word)) == Some(Decision::Stray)
}
