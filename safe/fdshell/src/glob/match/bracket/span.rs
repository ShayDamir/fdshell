//! Bracket-expression span: where a `[...]` ends, and the per-byte quote test
//! the span and membership scans share.

/// True when the byte at `i` was consumed inside double quotes.
pub(in crate::glob) fn quoted(mask: &[bool], i: usize) -> bool {
    mask.get(i).is_some_and(|&q| q)
}

/// End (one past the closing `]`) of the bracket expression opened by the
/// unquoted `[` at `at_open`, or `None` when the expression is unclosed.
pub(in crate::glob) fn span(bytes: &[u8], mask: &[bool], at_open: usize) -> Option<usize> {
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
            _ => i += 1,
        }
    }
    None
}
