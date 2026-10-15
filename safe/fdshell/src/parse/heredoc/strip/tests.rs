#![allow(clippy::unwrap_used)]
use super::strip_continuations;
use alloc::vec::Vec;
use sys::ShortCStr;

/// Strip a body, spelled with `\` bytes: every `\\` below is one backslash.
fn strip(text: &[u8]) -> Vec<u8> {
    strip_continuations(&ShortCStr::from_vec(text.to_vec()).unwrap())
        .unwrap()
        .as_bytes()
        .unwrap()
        .to_vec()
}

#[test]
fn continuation_pair_is_dropped() {
    // `a\<newline>b` → `ab`: both bytes of the pair go.
    assert_eq!(strip(b"a\\\nb\n"), b"ab\n");
    assert_eq!(strip(b"a\\\nb\\\n"), b"ab");
}

#[test]
fn escaped_backslash_before_a_newline_is_not_a_continuation() {
    // `a\\<newline>`: the pair is copied and the newline stays a body byte,
    // so the next line is a separate line (bash prints two lines).
    assert_eq!(strip(b"a\\\\\n"), b"a\\\\\n");
    assert_eq!(strip(b"a\\\\\nb\n"), b"a\\\\\nb\n");
}

#[test]
fn pairs_are_matched_left_to_right() {
    // `a\\\<newline>b`: the first two backslashes are a pair, the third pairs
    // with the newline, so the strip leaves `a\\b` and the substitution folds
    // the pair to `a\b` — bash's answer for the same body.
    assert_eq!(strip(b"a\\\\\\\nb\n"), b"a\\\\b\n");
    // `a\\\<newline>` with an even run: pairs only, the newline stays a body
    // byte.
    assert_eq!(strip(b"a\\\\\\\\\n"), b"a\\\\\\\\\n");
}

#[test]
fn trailing_backslash_is_copied() {
    // The end-of-body backslash belongs to the substitution, which keeps it.
    assert_eq!(strip(b"a\\"), b"a\\");
    assert_eq!(strip(b"\\"), b"\\");
}

#[test]
fn consecutive_continuations_collapse() {
    assert_eq!(strip(b"a\\\n\\\nb\n"), b"ab\n");
}

#[test]
fn other_escape_pairs_are_untouched() {
    // `\$`, `\\`, `` \` `` and every other `\<char>` are the substitution's
    // business: this pass copies them verbatim.
    assert_eq!(strip(b"a\\$b\nc\\`d\te\\\\f"), b"a\\$b\nc\\`d\te\\\\f");
}

#[test]
fn empty_body_stays_empty() {
    assert_eq!(strip(b""), b"");
}

#[test]
fn newline_only_body_is_untouched() {
    assert_eq!(strip(b"\n\n"), b"\n\n");
}
