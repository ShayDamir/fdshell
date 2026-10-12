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

/// The iterator form tracks the same span rule (the two variants share `Nest`).
#[test]
fn iterator_form_tracks_the_single_quote_span() {
    let mut bytes = b"echo 'a)b')".iter().copied().peekable();
    assert_eq!(scan_paren_body(&mut bytes, 1), Some(b"echo 'a)b'".to_vec()));
}
