//! Provenance of the bytes of a substituted word: which bytes came from an
//! expansion (POSIX 2.6.5: only those delimit a field), and where an
//! embedded `$@` positional starts a new word.
//!
//! The splitter walks the concatenated word text once; `splittable[i]` tells
//! it whether byte `i` was produced by an expansion (the gaps are the word's
//! literal bytes, which are always content), and `boundaries` marks the
//! positional starts. A boundary carries the positional's quote bit because
//! a **quoted** positional that ends the word keeps its field even when it
//! is empty (`IFS=,; set -- a ""; printf %s x"$@"` → `[xa][]`), while an
//! unquoted empty one vanishes (`[xa]`).

use alloc::vec;
use alloc::vec::Vec;

/// Word-boundary marks, `(byte index, positional quoted)`, pushed in
/// increasing index order (the `$@` arm appends positionals in order).
pub(crate) struct Origin {
    splittable: Vec<bool>,
    boundaries: Vec<(usize, bool)>,
}

impl Origin {
    /// No expansion ran: no byte delimits, no boundary cuts.
    pub(crate) fn new() -> Self {
        Self {
            splittable: Vec::new(),
            boundaries: Vec::new(),
        }
    }

    /// The origin of a text that *is* the expansion (the whole-word `$@`
    /// re-split): every byte splittable, no boundaries.
    pub(crate) fn all(len: usize) -> Self {
        Self {
            splittable: vec![true; len],
            boundaries: Vec::new(),
        }
    }

    /// Mark `[from, to)` as expansion bytes, padding the gap before `from`
    /// with literal bytes. `from == to` records nothing: an expansion arm
    /// that produced no bytes adds no splittable byte, which is the exact
    /// "may its bytes split?" signal the splitter needs.
    pub(crate) fn record(&mut self, from: usize, to: usize) {
        for _ in self.splittable.len()..from {
            self.splittable.push(false);
        }
        for _ in self.splittable.len()..to {
            self.splittable.push(true);
        }
    }

    /// A positional starts at byte index `at` and closes the open field.
    pub(crate) fn boundary(&mut self, at: usize, quoted: bool) {
        self.boundaries.push((at, quoted));
    }

    /// Pad the provenance to the final word length; the tail is literal.
    pub(crate) fn close(&mut self, len: usize) {
        for _ in self.splittable.len()..len {
            self.splittable.push(false);
        }
    }

    /// No byte of the word came from an expansion (a tilde arm deliberately
    /// records none).
    pub(crate) fn is_empty(&self) -> bool {
        !self.splittable.iter().any(|&s| s)
    }

    /// Whether byte `i` came from an expansion.
    pub(crate) fn splittable(&self, i: usize) -> bool {
        self.splittable.get(i).copied().unwrap_or(false)
    }

    /// The boundaries at byte `i`, in push order (adjacent empty
    /// positionals share an index).
    pub(crate) fn boundaries_at(&self, i: usize) -> &[(usize, bool)] {
        let start = self.boundaries.partition_point(|&(j, _)| j < i);
        let end = self.boundaries.partition_point(|&(j, _)| j <= i);
        self.boundaries.get(start..end).unwrap_or(&[])
    }

    /// The final field is a quoted positional's empty field when the last
    /// boundary sits at the word's end and carries the positional's quote
    /// bit: it survives the end-of-text flush.
    pub(crate) fn ends_quoted(&self, len: usize) -> bool {
        self.boundaries
            .last()
            .is_some_and(|&(j, quoted)| j == len && quoted)
    }
}

#[cfg(test)]
mod tests;
