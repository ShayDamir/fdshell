//! The **decision** layer of the pairing (task #436): what bash's grammar makes
//! of a closer word that the score of [`super`] cannot pair. The score stays a
//! score — an unpaired closer scores `0` and the block keeps its tail — while
//! this half reads the same stack, never a second table, and never a score
//! change. Task #467 brings the brace closer into the decision: bash refuses a
//! `}` that closes no open group (measured 2026-11-07, bash 5.3.9 rc 2, nothing
//! runs); the decision answers it like the other closers, and it is
//! [`Pairing::observe`] — the walk's recorder — that keeps not recording braces.

// mutants: -C --lib -- -E 'test(/^keywords::pairing::/) | test(/^comment::/)'

use super::Pairing;
use crate::keywords::Place;
use crate::keywords::kind::{Closer, Keyword, keyword_kind};

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
/// word data in bash too, and the place gate of [`crate::keywords::position`] owns
/// that half.
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

impl Pairing {
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
    ///
    /// The **brace** closer is not recorded here (task #467, the walk keeps not
    /// asking): this recorder serves the `comment` walk, whose pairing counts
    /// exact brace words only, so a glued definer's `{` keeps no depth and a
    /// legitimate closer of a glued body would read as stray. The walk answers
    /// the word with its own score; the run path of `segment.rs` asks
    /// [`Pairing::decision`] directly and reports the brace there.
    pub(crate) fn observe(&mut self, word: &[u8], line: &[u8], word_end: usize, place: Place) {
        if matches!(
            keyword_kind(word, line, word_end),
            Some(Keyword::Closes(Closer::Brace))
        ) {
            return;
        }
        if matches!(place, Place::Command)
            && self.top().is_none()
            && self.stray.is_none()
            && self.decision(word, line, word_end) == Some(Decision::Stray)
        {
            self.stray = Some(word_end - word.len());
        }
    }

    /// The [`Decision`] for one closer word, read **before** the word is
    /// [`Pairing::apply`]-ed — the same read [`crate::keywords::position`] makes when it
    /// takes [`Pairing::top`] as the pairing gate of the closer row. `None` answers
    /// a word that is no keyword (no closer, no decision) and an opener (it opens
    /// a block, it closes nothing); every closer is asked, the brace like the
    /// others (task #467: bash 5.3.9 refuses a `}` that closes no open group,
    /// rc 2, nothing runs), and the answer is `Stray` for both unpaired arms of
    /// [`Pairing::apply`]: the kind-mismatch arm and the arm of a closer seen with
    /// nothing open.
    ///
    /// Task #436's rule is asked **only** by the top-level scan, at words the
    /// place gate puts in command position; the nested scorers read the same
    /// pairing for their score and must not report the same word twice. For the
    /// `comment` walk the brace stays unasked at the recorder
    /// ([`Pairing::observe`], task #467); the run path of `segment.rs` asks it
    /// directly.
    pub(crate) fn decision(&self, word: &[u8], line: &[u8], word_end: usize) -> Option<Decision> {
        let kind = match keyword_kind(word, line, word_end) {
            Some(Keyword::Closes(kind)) => kind,
            _ => return None,
        };
        if self.stack.last() == Some(&kind) {
            return Some(Decision::Paired);
        }
        Some(Decision::Stray)
    }
}
