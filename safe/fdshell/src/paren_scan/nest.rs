//! The quote/paren state of a parenthesized body scan (task #141).
//!
//! Shared by the iterator variant (`scan_paren_body`) and the slice variant
//! (`scan_at`) so the two cannot diverge (LESSONS: byte-level and token-level
//! rules must agree). A `'…'` span is opaque: its `(`/`)`/`"` are data and a
//! backslash written in it is literal (POSIX §2.2.3), so only a `"` outside
//! both spans counts as a quote toggle.

use alloc::vec::Vec;

use crate::bytes::{QUOTE, SQUOTE};

/// Running depth/quote state of one body scan. `base` is the depth the caller
/// already consumed; the body ends at the first `)` bringing the count back to
/// `base`.
pub(super) struct Nest {
    depth: u32,
    base: u32,
    double: bool,
    single: bool,
}

impl Nest {
    pub(super) fn new(base: u32) -> Self {
        Self {
            depth: base,
            base,
            double: false,
            single: false,
        }
    }

    /// `true` when `c` closes the body.
    pub(super) fn closes(&self, c: u8) -> bool {
        c == b')' && !self.double && !self.single && self.depth == self.base
    }

    /// `true` when the scan is inside a `'…'` span: the byte at hand is span
    /// data, so the caller must not apply its escape-pair rule to it.
    pub(super) fn opaque(&self) -> bool {
        self.single
    }

    /// Push `c` into `out` and fold it into the state.
    pub(super) fn consume(&mut self, out: &mut Vec<u8>, c: u8) {
        out.push(c);
        if self.single {
            self.single = c != SQUOTE;
            return;
        }
        if c == SQUOTE && !self.double {
            self.single = true;
            return;
        }
        match c {
            QUOTE => self.double = !self.double,
            b'(' if !self.double => self.depth = self.depth.saturating_add(1),
            b')' if !self.double => self.depth = self.depth.saturating_sub(1),
            _ => {}
        }
    }
}
