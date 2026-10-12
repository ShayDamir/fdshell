use alloc::vec;

use super::{scan_at, scan_paren_body};

#[test]
fn escaped_close_paren_stays_in_the_body() {
    // The caller consumed `(`; the pair `\)` is data, the final `)` ends it.
    let got = scan_at(b"a\\)b)", 0, 1);
    assert_eq!(got, Some((vec![b'a', b'\\', b')', b'b'], 5)));
}

#[test]
fn escaped_open_paren_never_increases_depth() {
    let got = scan_at(b"a\\(b)", 0, 1);
    assert_eq!(got, Some((vec![b'a', b'\\', b'(', b'b'], 5)));
}

#[test]
fn escaped_quote_never_toggles_quote_state() {
    // If `\"` toggled the quote state, the final `)` would be data and the
    // scan would return `None`.
    let got = scan_at(b"a\\\"b)", 0, 1);
    assert_eq!(got, Some((vec![b'a', b'\\', b'"', b'b'], 5)));
}

#[test]
fn unescaped_parens_still_track_depth() {
    assert_eq!(scan_at(b"a(b)c)", 0, 1), Some((b"a(b)c".to_vec(), 6)));
    // A quote makes the parens data.
    assert_eq!(scan_at(b"\"a)b\"", 0, 1), None);
    assert_eq!(scan_at(b"a(b", 0, 1), None);
}

#[test]
fn iterator_form_skips_the_escape_pair() {
    let mut bytes = [b'a', b'\\', b')', b'b', b')'].into_iter().peekable();
    let got = scan_paren_body(&mut bytes, 1);
    assert_eq!(got, Some(vec![b'a', b'\\', b')', b'b']));
    // The closing paren is consumed, so the iterator is drained after it.
    assert_eq!(bytes.next(), None);
}

#[test]
fn trailing_backslash_never_closes_the_body() {
    // The pair needs a second byte: a trailing `\` ends the scan with `None`.
    assert_eq!(scan_at(b"a\\", 0, 1), None);
    let mut bytes = [b'a', b'\\'].into_iter().peekable();
    assert_eq!(scan_paren_body(&mut bytes, 1), None);
}

/// A `'…'` span is opaque (task #141): the `(`, `)` and `"` written in it are
/// body data, so `$(echo 'a)b')` has the body `echo 'a)b'` and the closing
/// paren is the one outside the span.
#[test]
fn single_quoted_span_is_body_data() {
    assert_eq!(
        scan_at(b"echo 'a)b')", 0, 1),
        Some((b"echo 'a)b'".to_vec(), 11))
    );
    // The span's own `(` counts no depth.
    assert_eq!(
        scan_at(b"echo 'a(b')", 0, 1),
        Some((b"echo 'a(b'".to_vec(), 11))
    );
    // An unclosed span swallows the closing paren.
    assert_eq!(scan_at(b"echo 'a)b", 0, 1), None);
}

/// A backslash written inside a `'…'` span is literal (POSIX §2.2.3): it does
/// not shield the `)` behind it, so the span ends the body scan there.
#[test]
fn span_backslash_shields_nothing() {
    assert_eq!(
        scan_at(b"echo 'a\\b')", 0, 1),
        Some((b"echo 'a\\b'".to_vec(), 11))
    );
    // `')` inside the span: the paren is data, and the span never closes.
    assert_eq!(scan_at(b"echo '\\a)b'", 0, 1), None);
}

/// The span's opacity is what disables the escape-pair rule inside it. With a
/// `\` in a span the pair is never formed, so (a) the `)` written behind it is
/// span data and the body ends at the span's own closing quote plus the real
/// `)` — `$(echo 'a\)b')` has the body `echo 'a\)b'` (bash prints `a\)b`) — and
/// (b) the closing quote is never swallowed as a pair's second byte:
/// `$(echo 'x\')` has the body `echo 'x\'` and ends at the `)` after the span
/// (bash prints `x\`).
#[test]
fn span_backslash_leaves_the_escape_arm_out() {
    assert_eq!(
        scan_at(b"echo 'a\\)b')", 0, 1),
        Some((b"echo 'a\\)b'".to_vec(), 12)),
        "the `\\)` pair is not formed inside a span: the `)` is span data"
    );
    assert_eq!(
        scan_at(b"echo 'x\\')", 0, 1),
        Some((b"echo 'x\\'".to_vec(), 10)),
        "the span's closing quote is not a pair tail, so the span closes and the `)` ends the body"
    );
    let mut bytes = b"echo 'x\\')".iter().copied().peekable();
    assert_eq!(
        scan_paren_body(&mut bytes, 1),
        Some(b"echo 'x\\'".to_vec()),
        "the iterator form takes the same body"
    );
}

/// The iterator form tracks the same span rule (the two variants share `Nest`).
#[test]
fn iterator_form_tracks_the_single_quote_span() {
    let mut bytes = b"echo 'a)b')".iter().copied().peekable();
    assert_eq!(scan_paren_body(&mut bytes, 1), Some(b"echo 'a)b'".to_vec()));
}
