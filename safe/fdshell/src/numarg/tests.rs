//! The per-grammar matrix: one test per grammar covering whitespace, sign,
//! leading zeros, fraction, trailing junk and base prefixes — every row
//! measured against bash 5.3.9 (task #600).

use super::*;

#[test]
fn legal_number_accepts_its_grammar() {
    assert_eq!(decimal(b" 1", &LEGAL_NUMBER), Some(1));
    assert_eq!(decimal(b"1 ", &LEGAL_NUMBER), Some(1));
    assert_eq!(decimal(b"+1", &LEGAL_NUMBER), Some(1));
    assert_eq!(decimal(b"010", &LEGAL_NUMBER), Some(10));
    assert_eq!(decimal(b"-0", &LEGAL_NUMBER), Some(0));
    assert_eq!(decimal(b"9", &LEGAL_NUMBER), Some(9));
}

#[test]
fn legal_number_rejects_outside_its_grammar() {
    for text in ["1x", "0x2", "abc", "+", ""] {
        assert_eq!(decimal(text.as_bytes(), &LEGAL_NUMBER), None, "{text:?}");
    }
}

#[test]
fn legal_number_fits_sites_and_the_scanner_bounds_at_i128() {
    // 1e20 fits `i128`: the scanner answers `Some` and each call site applies
    // its own range; the 40-digit run past `i128` the scanner itself refuses.
    assert_eq!(
        decimal(b"100000000000000000000", &LEGAL_NUMBER),
        Some(100_000_000_000_000_000_000)
    );
    assert_eq!(
        decimal(b"99999999999999999999999999999999999999999", &LEGAL_NUMBER),
        None
    );
}

#[test]
fn ulimit_value_accepts_decimal_digit_runs() {
    assert_eq!(decimal(b"0100", &ULIMIT_VALUE), Some(100));
    assert_eq!(decimal(b"010", &ULIMIT_VALUE), Some(10));
}

#[test]
fn ulimit_value_rejects_whitespace_sign_and_junk() {
    for text in [
        "+5",
        " 5",
        "5 ",
        "1x",
        "99999999999999999999999999999999999999999",
    ] {
        assert_eq!(decimal(text.as_bytes(), &ULIMIT_VALUE), None, "{text:?}");
    }
}

#[test]
fn seconds_accepts_its_fractional_grammar() {
    assert_eq!(seconds_ms(b"0.5"), Some(500));
    assert_eq!(seconds_ms(b".5"), Some(500));
    assert_eq!(seconds_ms(b"1.0"), Some(1000));
    assert_eq!(seconds_ms(b"1."), Some(1000));
    assert_eq!(seconds_ms(b"+.5"), Some(500));
    assert_eq!(seconds_ms(b"0"), Some(0));
    assert_eq!(seconds_ms(b"9"), Some(9000));
    assert_eq!(seconds_ms(b"0.9"), Some(900));
}

#[test]
fn seconds_rejects_outside_its_grammar() {
    for text in [" 1", "-1", "-0.5", "1e1", "1,5", "0x1", "1.5x"] {
        assert_eq!(seconds_ms(text.as_bytes()), None, "{text:?}");
    }
}
