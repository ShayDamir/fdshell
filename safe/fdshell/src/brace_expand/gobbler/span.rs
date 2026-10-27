//! The gobbler's quote-span state machine: bytes inside a quote span are
//! data. Inside a `'…'` span there is no escape pair (POSIX §2.2.3 — a
//! backslash is literal there), so only the closing quote ends the span
//! (task #141).

use crate::bytes::{QUOTE, SQUOTE};

/// Which quote span the gobbler sits inside, if any.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Span {
    None,
    Double,
    Single,
    Backtick,
}

impl Span {
    /// Whether the scanner is currently inside a quote span.
    pub(super) fn inside(self) -> bool {
        self != Self::None
    }

    /// Open a span when `c` is an opening quote byte at the top level;
    /// returns whether a span was opened.
    pub(super) fn open(&mut self, c: u8) -> bool {
        let span = match c {
            QUOTE => Self::Double,
            SQUOTE => Self::Single,
            b'`' => Self::Backtick,
            _ => return false,
        };
        *self = span;
        true
    }

    /// Consume one byte inside a span: `+2` over an escape pair, `+1`
    /// otherwise, clearing the state on the matching closer.
    pub(super) fn step(&mut self, c: u8, i: usize) -> usize {
        let (closer, escapes) = match *self {
            Self::Double => (QUOTE, true),
            Self::Single => (SQUOTE, false),
            Self::Backtick => (b'`', true),
            Self::None => return i + 1, // the caller steps only when `inside()`
        };
        if c == b'\\' && escapes {
            i + 2
        } else if c == closer {
            *self = Self::None;
            i + 1
        } else {
            i + 1
        }
    }
}
