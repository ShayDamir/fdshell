//! Bracket-expression span: where a `[...]` ends, and the prologue and the
//! per-byte quote test the span and membership scans share.

use crate::mask::{MaskBit, quoted_at};

/// True when the byte at `i` was consumed inside quotes. A bit past the mask is
/// unquoted, so a mask shorter than the pattern leaves the rest unquoted.
pub(in crate::glob) fn quoted(mask: &[MaskBit], i: usize) -> bool {
    quoted_at(mask, i)
}

/// The bracket prologue the span and membership scans share: the index of the
/// expression's first member byte, and whether the expression is negated. bash
/// accepts an unquoted `!` **or** `^` right after `[`; POSIX XCU 2.13.2 defines
/// only `!`, so the `^` arm is bash parity, not a divergence (LESSONS.md). A
/// quoted marker is a literal member, so the quote mask gates the test.
pub(in crate::glob) fn negation(bytes: &[u8], mask: &[MaskBit], at_open: usize) -> (usize, bool) {
    let i = at_open + 1;
    if matches!(bytes.get(i), Some(&b'!' | b'^')) && !quoted(mask, i) {
        return (i + 1, true);
    }
    (i, false)
}

/// End (one past the closing `]`) of the bracket expression opened by the
/// unquoted `[` at `at_open`, or `None` when the expression is unclosed.
pub(in crate::glob) fn span(bytes: &[u8], mask: &[MaskBit], at_open: usize) -> Option<usize> {
    let (mut i, _) = negation(bytes, mask, at_open);
    if bytes.get(i) == Some(&b']') {
        i += 1;
    }
    while let Some(&b) = bytes.get(i) {
        match b {
            b'\\' if !quoted(mask, i) => i += 2,
            b']' if !quoted(mask, i) => return Some(i + 1),
            // The inner `]` of a `[:class:]` must not close the expression.
            b'[' if super::class::is_class_start(bytes, mask, i) => {
                i = super::class::class_end(bytes, i).unwrap_or(i + 1);
            }
            _ => i += 1,
        }
    }
    None
}
