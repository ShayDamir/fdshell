#![allow(clippy::unwrap_used)]
use alloc::vec;
use alloc::vec::Vec;

use super::clobber_pipe;
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
    assert!(clobber_pipe(b"a>|b", 2), "`>|` is an operator");
    assert!(
        !clobber_pipe(b"a'>|b", 4),
        "the `>` is inside a span: the `|` is a pipe"
    );
    assert!(
        !clobber_pipe(b"a\">|b", 4),
        "the double-quoted case, unchanged"
    );
    // The `'` written inside `"` is data: the `>` is masked, so the `|` is a
    // pipe (`echo "a'b>"c` prints `a'b>c`).
    assert!(
        !clobber_pipe(b"a\"'b>\"|c", 6),
        "the `'` in `\"` opens no span, the `>` stays quoted"
    );
}

/// The three quote arms are mutually gated, and each gate is visible in the
/// mask byte by byte: a `"` written in a span never toggles (the bytes after
/// the span are bare again), a `'` written in `"` never opens a span, and no
/// escape pair is formed inside a span (so the span's bytes are one mask entry
/// each and the `>` written in it is a masked byte, not a pair's tail).
#[test]
fn each_quote_arm_is_gated_by_the_other_state() {
    // `'a"b'c`: the `"` is span data, `c` is bare again.
    assert_eq!(
        quoted(b"'a\"b'c"),
        vec![false, true, true, true, true, false]
    );
    // `"a'b"c>`: the `'` opens nothing, so `c`/`>` are bare after the `"`.
    assert_eq!(
        quoted(b"\"a'b\"c>"),
        vec![false, true, true, true, true, false, false]
    );
    // `'a\>'`: 5 source bytes, 5 mask entries — no pair consumed the `>`.
    assert_eq!(quoted(b"'a\\>'"), vec![false, true, true, true, true]);
    // The unquoted baseline keeps its pair: `\>` is two bare bytes.
    assert_eq!(quoted(b"a\\>"), vec![false, false, false]);
}

/// The escape arm is the only place that consumes a pair, and its gate is what
/// makes a quoted escape pair invisible to the quote toggles: in `a\"b` the pair
/// is consumed whole, so the `"` never opens a double quote and every byte
/// stays bare, while the same bytes without the backslash quote the `b`.
#[test]
fn escape_pair_never_leaves_a_toggle_behind() {
    // `a\"b`: the pair is two bare bytes and the `"` is its tail, not a toggle.
    assert_eq!(quoted(b"a\\\"b"), vec![false, false, false, false]);
    // Without the backslash the `"` opens a span: the mask is parallel to the
    // bytes, one entry each.
    assert_eq!(quoted(b"a\"b"), vec![false, false, true]);
    // A `\'` written in `"` is span data (`"` gates the pair), so the closing
    // `"` still closes the span and the `b` after it is bare.
    assert_eq!(
        quoted(b"\"a\\'\"b"),
        vec![false, true, true, true, true, false]
    );
}
