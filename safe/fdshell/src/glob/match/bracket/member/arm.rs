//! The arms of one member turn: each inspects the bytes that follow the
//! member's first byte and answers with a `Step`, or `None` when the arm's
//! shape is not there and the next arm (finally the plain member) applies.

use super::super::{class, quoted};
use super::{Members, Step};

impl<'a> Members<'a> {
    /// The `[:class:]` arm: a recognized class is one atomic member; an
    /// unrecognized one (or an unclosed `[:`) falls through (`None`) and its
    /// bytes are literal members.
    pub(super) fn class(&mut self, b: u8) -> Option<Step> {
        let (close, member) = class::class_member(self.bytes, self.mask, self.i, b)?;
        if member {
            return Some(Step::Matched);
        }
        // A `]` right after the class closes the expression (the class
        // was its last member); without this the scan would leak past
        // the bracket and claim trailing bytes as members.
        let Some(nxt) = self.bytes.get(close).copied() else {
            return Some(Step::Stop);
        };
        if nxt == b']' && !quoted(self.mask, close) {
            return Some(Step::Stop);
        }
        self.c = nxt;
        self.i = close + 1;
        Some(Step::Next)
    }

    /// The `a-z` range arm: `nxt` (at `dash_i`) follows the member's first
    /// byte, so an unquoted, unescaped `-` there makes the next byte `e` the
    /// range end. An invalid range (start after end) matches nothing, and
    /// scanning stops when the byte right after the range end is `]`,
    /// otherwise that byte is the next member (bash `BRACKMATCH` semantics).
    /// `None` when these bytes form no range; `nxt` is then the next member.
    pub(super) fn range(&mut self, b: u8, nxt: u8, dash_i: usize, plain: bool) -> Option<Step> {
        let e = self.bytes.get(self.i).copied()?;
        if e == b']'
            || quoted(self.mask, self.i)
            || e == b'\\'
            || nxt != b'-'
            || quoted(self.mask, dash_i)
            || !plain
        {
            return None;
        }
        self.end = e;
        self.i += 1;
        let Some(after) = self.bytes.get(self.i).copied() else {
            return Some(Step::Stop);
        };
        self.i += 1;
        if self.start > self.end {
            if after == b']' {
                return Some(Step::Stop);
            }
            self.c = after;
            return Some(Step::Next);
        }
        if self.start <= b && b <= self.end {
            return Some(Step::Matched);
        }
        if after == b']' {
            return Some(Step::Stop);
        }
        self.c = after;
        Some(Step::Next)
    }

    /// The plain member left after the arms: `start`/`end` is the member, and
    /// an unquoted `nxt` of `]` closes the expression.
    pub(super) fn tail(&mut self, b: u8, nxt: u8, dash_i: usize) -> Step {
        if self.start <= b && b <= self.end {
            return Step::Matched;
        }
        if nxt == b']' && !quoted(self.mask, dash_i) {
            return Step::Stop;
        }
        self.c = nxt;
        Step::Next
    }
}
