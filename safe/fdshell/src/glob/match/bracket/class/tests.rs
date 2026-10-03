#![allow(clippy::unwrap_used)]

use super::{class_contains, class_end, is_class_start};

#[test]
fn is_class_start_detects_unquoted_colon() {
    assert!(is_class_start(b"[:alpha:]]", &[], 0));
    assert!(is_class_start(b"x[:digit:]", &[], 1));
    // A `[` not followed by `:` is not a class start.
    assert!(!is_class_start(b"[a]", &[], 0));
    // A quoted `[` is literal, not a class start.
    assert!(!is_class_start(
        b"[:alpha:]]",
        &[true, false, false, false, false, false, false],
        0
    ));
}

#[test]
fn class_end_finds_closing_bracket() {
    assert_eq!(class_end(b"[:alpha:]", 0), Some(9));
    assert_eq!(class_end(b"x[:digit:]", 1), Some(10));
    // No closing `:]`: not a class.
    assert_eq!(class_end(b"[:alpha", 0), None);
    // A `]` without the preceding `:` is not the close.
    assert_eq!(class_end(b"[:a]lpha:", 0), None);
}

#[test]
fn alpha_class() {
    assert_eq!(class_contains(b"alpha", b'a'), Some(true));
    assert_eq!(class_contains(b"alpha", b'Z'), Some(true));
    assert_eq!(class_contains(b"alpha", b'5'), Some(false));
    assert_eq!(class_contains(b"alphabetic", b'g'), Some(true));
}

#[test]
fn digit_alnum_classes() {
    assert_eq!(class_contains(b"digit", b'0'), Some(true));
    assert_eq!(class_contains(b"digit", b'9'), Some(true));
    assert_eq!(class_contains(b"digit", b'a'), Some(false));
    assert_eq!(class_contains(b"numeric", b'7'), Some(true));
    assert_eq!(class_contains(b"alnum", b'k'), Some(true));
    assert_eq!(class_contains(b"alnum", b'3'), Some(true));
    assert_eq!(class_contains(b"alnum", b' '), Some(false));
}

#[test]
fn upper_lower_classes() {
    assert_eq!(class_contains(b"upper", b'Q'), Some(true));
    assert_eq!(class_contains(b"upper", b'q'), Some(false));
    assert_eq!(class_contains(b"uppercase", b'M'), Some(true));
    assert_eq!(class_contains(b"lower", b'z'), Some(true));
    assert_eq!(class_contains(b"lowercase", b'B'), Some(false));
}

#[test]
fn xdigit_class() {
    assert_eq!(class_contains(b"xdigit", b'9'), Some(true));
    assert_eq!(class_contains(b"xdigit", b'F'), Some(true));
    assert_eq!(class_contains(b"xdigit", b'f'), Some(true));
    assert_eq!(class_contains(b"hexdigit", b'g'), Some(false));
}

#[test]
fn space_blank_cntrl_classes() {
    assert_eq!(class_contains(b"space", b' '), Some(true));
    assert_eq!(class_contains(b"space", b'\t'), Some(true));
    assert_eq!(class_contains(b"space", b'x'), Some(false));
    assert_eq!(class_contains(b"blank", b' '), Some(true));
    assert_eq!(class_contains(b"blank", b'\n'), Some(false));
    assert_eq!(class_contains(b"cntrl", b'\n'), Some(true));
    assert_eq!(class_contains(b"control", 0x1F), Some(true));
    assert_eq!(class_contains(b"cntrl", b'a'), Some(false));
}

#[test]
fn graph_print_del_punct_classes() {
    assert_eq!(class_contains(b"graph", b'a'), Some(true));
    assert_eq!(class_contains(b"graph", b'~'), Some(true));
    assert_eq!(class_contains(b"graph", b' '), Some(false));
    assert_eq!(class_contains(b"print", b' '), Some(true));
    assert_eq!(class_contains(b"printable", b'~'), Some(true));
    assert_eq!(class_contains(b"print", 0x7F), Some(false));
    assert_eq!(class_contains(b"del", 0x7F), Some(true));
    assert_eq!(class_contains(b"del", b'a'), Some(false));
    assert_eq!(class_contains(b"punct", b'-'), Some(true));
    assert_eq!(class_contains(b"punctuation", b'9'), Some(false));
    assert_eq!(class_contains(b"punct", b'a'), Some(false));
}

#[test]
fn unknown_class_is_none() {
    assert_eq!(class_contains(b"bogus", b'a'), None);
    assert_eq!(class_contains(b"", b'a'), None);
}
