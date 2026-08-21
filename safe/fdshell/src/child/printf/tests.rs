#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::render;

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn rendered(fmt: &str, args: &[&str]) -> String {
    let cs: Vec<CString> = args.iter().map(|a| c(a)).collect();
    let refs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let mut out = Vec::new();
    render(fmt.as_bytes(), &refs, &mut out).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn string_and_number() {
    assert_eq!(rendered("%s=%d", &["a", "7"]), "a=7");
}

#[test]
fn percent_escape() {
    assert_eq!(rendered("100%%", &[]), "100%");
}

#[test]
fn missing_string_is_empty() {
    assert_eq!(rendered("[%s]", &[]), "[]");
}

#[test]
fn missing_number_is_zero() {
    assert_eq!(rendered("[%d]", &[]), "[0]");
}

#[test]
fn format_reused_while_args_remain() {
    assert_eq!(rendered("%s,", &["a", "b", "c"]), "a,b,c,");
}

#[test]
fn format_without_conversions_prints_once() {
    assert_eq!(rendered("x", &["a", "b"]), "x");
}

#[test]
fn unsigned_octal_hex() {
    assert_eq!(
        rendered("%u %o %x %X", &["10", "10", "255", "255"]),
        "10 12 ff FF"
    );
}

#[test]
fn signed_i_conversion() {
    assert_eq!(rendered("[%i]", &["-3"]), "[-3]");
}

#[test]
fn negative_number_two_complement_for_base_conversions() {
    assert_eq!(
        rendered("%o %x", &["-1", "-1"]),
        "1777777777777777777777 ffffffffffffffff"
    );
}

#[test]
fn char_is_first_byte() {
    assert_eq!(rendered("[%c]", &["hello"]), "[h]");
}

#[test]
fn char_without_arg_is_empty() {
    assert_eq!(rendered("[%c]", &[]), "[]");
}

#[test]
fn char_empty_arg_is_empty() {
    assert_eq!(rendered("[%c]", &[""]), "[]");
}

#[test]
fn unknown_conversion_printed_as_is() {
    assert_eq!(rendered("%z", &["a"]), "%z");
}

#[test]
fn trailing_percent_alone() {
    assert_eq!(rendered("a%", &[]), "a%");
}

#[test]
fn backslash_escapes_in_format() {
    // Raw strings: the format must contain literal `\x` sequences.
    assert_eq!(rendered(r"a\nb\tc\r", &[]), "a\nb\tc\r");
}

#[test]
fn all_named_escapes() {
    assert_eq!(
        rendered(r"\n\t\r\a\b\f\v\\", &[]),
        "\n\t\r\u{7}\u{8}\u{c}\u{b}\\"
    );
}

#[test]
fn octal_escape() {
    assert_eq!(rendered(r"\101\102", &[]), "AB");
}

#[test]
fn octal_escape_stops_at_three_digits() {
    assert_eq!(rendered(r"\1011", &[]), "A1");
}

#[test]
fn octal_escape_zero_is_nul() {
    let mut out = Vec::new();
    render(b"\0", &[], &mut out).unwrap();
    assert_eq!(out, vec![0u8]);
}

#[test]
fn unknown_escape_printed_as_is() {
    assert_eq!(rendered(r"\q", &[]), "\\q");
}

#[test]
fn trailing_backslash_alone() {
    assert_eq!(rendered("a\\", &[]), "a\\");
}

#[test]
fn invalid_number_is_error() {
    let cs = c("abc");
    let mut out = Vec::new();
    let e = render(b"%d", core::slice::from_ref(&cs.as_c_str()), &mut out).unwrap_err();
    assert!(matches!(
        e.current_context(),
        builtins::error::BuiltinError::InvalidArgument("number")
    ));
}
