//! The membership scan of one bracket expression: the cursor state lives in
//! the `Members` scanner, one member per loop turn.
//!
//! The loop keeps the invariant `c == bytes[i - 1]`: `c` is the current
//! member's first byte and `i` the byte right after it (LESSONS: "A dense
//! scanner split into a `&mut self` newtype must keep every arm's order and
//! cursor advance"), so the arms' order and cursor advances must stay exactly
//! as they are. The class arm's `close + 1` is load-bearing at the membership
//! surface: resuming at `close` re-reads the byte after the class as a range
//! start, and
//! `glob/match/tests.rs::class_then_range_keeps_the_cursor_one_past_the_class`
//! is the row that fails.

mod arm;

use crate::mask::MaskBit;

use super::{negation, quoted};

/// How one member turn leaves the scan: `b` is a member, the expression ended,
/// or the next member is loaded and the loop continues.
pub(super) enum Step {
    Matched,
    Stop,
    Next,
}

/// The scan state of a bracket expression: its bytes, the quote mask, the
/// cursor `i` (just past the current member's first byte `c`), the current
/// member's bounds `start`/`end`, and the leading-`!`/`^` negation.
pub(super) struct Members<'a> {
    bytes: &'a [u8],
    mask: &'a [MaskBit],
    i: usize,
    c: u8,
    start: u8,
    end: u8,
    negated: bool,
}

impl<'a> Members<'a> {
    /// Whether `b` is a member of the (valid) bracket expression at `at_open`.
    ///
    /// The prologue: an unquoted `!` or `^` right after `[` negates the
    /// expression; the byte after it is the first member. An expression with
    /// no member byte at all answers with its own negation flag.
    pub(super) fn scan(bytes: &'a [u8], mask: &'a [MaskBit], at_open: usize, b: u8) -> bool {
        let (i, negated) = negation(bytes, mask, at_open);
        let Some(c) = bytes.get(i).copied() else {
            return negated;
        };
        let members = Self {
            bytes,
            mask,
            i: i + 1,
            c,
            start: c,
            end: c,
            negated,
        };
        members.run(b)
    }

    /// The driver loop: one member per turn, until a member matches or the
    /// expression ends.
    fn run(mut self, b: u8) -> bool {
        loop {
            return match self.member(b) {
                Step::Matched => !self.negated,
                Step::Stop => self.negated,
                Step::Next => continue,
            };
        }
    }

    /// One member turn, starting at `c` (the member's first byte). The arms
    /// run in a fixed order: `\X` escape, `[:class:]`, `a-z` range, then the
    /// plain-member test of `arm::tail`.
    fn member(&mut self, b: u8) -> Step {
        self.start = self.c;
        self.end = self.c;
        let mut plain = !quoted(self.mask, self.i - 1);
        if plain && self.c == b'\\' {
            let Some(n) = self.bytes.get(self.i).copied() else {
                return Step::Stop;
            };
            self.start = n;
            self.end = n;
            plain = false;
            self.i += 1;
        }
        if plain
            && self.c == b'['
            && let Some(step) = self.class(b)
        {
            return step;
        }
        let Some(nxt) = self.bytes.get(self.i).copied() else {
            return Step::Stop;
        };
        let dash_i = self.i;
        self.i += 1;
        if let Some(step) = self.range(b, nxt, dash_i, plain) {
            return step;
        }
        self.tail(b, nxt, dash_i)
    }
}
