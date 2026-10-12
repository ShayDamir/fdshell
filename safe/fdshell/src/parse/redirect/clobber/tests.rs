#![allow(clippy::unwrap_used)]
use alloc::vec;
use alloc::vec::Vec;

use super::quote_mask;

/// All-false mask of a byte run, for the "is this byte quoted?" reads.
fn quoted(line: &[u8]) -> Vec<bool> {
    quote_mask(line, line.len())
}

/// A `'…'` span masks every byte after its opening quote (task #141): the mask
/// bit is pushed before the toggle, so the opening marker is `false` and the
/// closing marker is `true` — exactly the double-quote convention.
#[test]
fn single_quote_span_masks_its_bytes() {
    // `a'bcd'e`: the opening marker is unquoted, the span bytes and the
    // closing marker are `true` (the mask is pushed before the toggle).
    assert_eq!(
        quoted(b"a'bcd'e"),
        vec![false, false, true, true, true, true, false]
    );
    // `''` is an empty span: only the marker bytes, the closing one masked.
    assert_eq!(quoted(b"''"), vec![false, true]);
    // A `"` written inside a span is data (mask `true`), not a toggle.
    assert_eq!(quoted(b"'a\"b'"), vec![false, true, true, true, true]);
}

/// The span and the double-quote state mask independently; an unquoted escape
/// pair is two `false` bytes, and inside a span a backslash is literal, so it
/// shields nothing (`'a\>b'` masks the `>`).
#[test]
fn span_masks_the_escape_and_the_quote_bytes() {
    // Unquoted escape pair: `\>` is two unquoted bytes.
    assert_eq!(quoted(b"a\\>b"), vec![false, false, false, false]);
    // In a span the backslash is one data byte: every span byte is masked.
    assert_eq!(
        quoted(b"'a\\>b'"),
        vec![false, true, true, true, true, true]
    );
    // A span inside double quotes: the `'` is a data byte of the outer span.
    assert_eq!(quoted(b"\"a'b\""), vec![false, true, true, true, true]);
}

/// `clobber_pipe`: a `|` right after a `'…'` span ending in `>` is a pipeline
/// pipe, exactly as after a double-quoted one (`'a>'|b` is a pipeline).
#[test]
fn span_terminated_by_operator_byte_is_a_pipe() {
    assert!(
        crate::parse::clobber_pipe(b"a>|b", 2),
        "`>|` is an operator"
    );
    assert!(
        !crate::parse::clobber_pipe(b"a'>|b", 4),
        "the `>` is inside a span: the `|` is a pipe"
    );
    assert!(
        !crate::parse::clobber_pipe(b"a\">|b", 4),
        "the double-quoted case, unchanged"
    );
}
