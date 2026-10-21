//! Bracket-expression span: where a `[...]` ends, and the per-byte quote test
//! the span and membership scans share.

use crate::mask::{MaskBit, quoted_at};

/// True when the byte at `i` was consumed inside quotes. A bit past the mask is
/// unquoted, so a mask shorter than the pattern leaves the rest unquoted.
pub(in crate::glob) fn quoted(mask: &[MaskBit], i: usize) -> bool {
    quoted_at(mask, i)
}

/// End (one past the closing `]`) of the bracket expression opened by the
/// unquoted `[` at `at_open`, or `None` when the expression is unclosed.
pub(in crate::glob) fn span(bytes: &[u8], mask: &[MaskBit], at_open: usize) -> Option<usize> {
    let mut i = at_open + 1;
    if bytes.get(i) == Some(&b'!') && !quoted(mask, i) {
        i += 1;
    }
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
