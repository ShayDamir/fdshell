#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::Sink;
use super::render;

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn refs(args: &[&str]) -> Vec<CString> {
    args.iter().map(|a| c(a)).collect()
}

fn rendered_pair(fmt: &str, args: &[&str]) -> (String, String, bool) {
    let cs = refs(args);
    let strs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let mut sink = Sink::new();
    render(fmt.as_bytes(), &strs, &mut sink).unwrap();
    (
        String::from_utf8(sink.out).unwrap(),
        String::from_utf8(sink.err).unwrap(),
        sink.failed,
    )
}

fn rendered(fmt: &str, args: &[&str]) -> String {
    rendered_pair(fmt, args).0
}

fn render_err(fmt: &str, args: &[&str]) -> error_stack::Report<builtins::error::BuiltinError> {
    let cs = refs(args);
    let strs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let mut sink = Sink::new();
    render(fmt.as_bytes(), &strs, &mut sink).unwrap_err()
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
fn char_without_arg_is_nul() {
    let mut out = Vec::new();
    let mut sink = Sink::new();
    render(b"[%c]", &[], &mut sink).unwrap();
    out.extend_from_slice(&sink.out);
    assert_eq!(out, vec![b'[', 0, b']']);
}

#[test]
fn char_empty_arg_is_nul() {
    let (out, _, _) = rendered_pair("[%c]", &[""]);
    assert_eq!(out, "[\0]");
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
    let (out, _, _) = rendered_pair(r"\0", &[]);
    assert_eq!(out, "\u{0}");
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
fn unknown_conversion_is_error() {
    assert!(matches!(
        render_err("%z", &["a"]).current_context(),
        builtins::error::BuiltinError::InvalidFormat
    ));
}

#[test]
fn trailing_percent_is_error() {
    assert!(matches!(
        render_err("a%", &[]).current_context(),
        builtins::error::BuiltinError::InvalidFormat
    ));
}

#[test]
fn incomplete_specifier_is_error() {
    assert!(matches!(
        render_err("%.", &[]).current_context(),
        builtins::error::BuiltinError::InvalidFormat
    ));
}

#[test]
fn invalid_number_reported_to_stderr() {
    let (out, err, failed) = rendered_pair("[%d]", &["abc"]);
    assert_eq!(out, "[0]");
    assert_eq!(err, "printf: abc: invalid number\n");
    assert!(failed);
}

#[test]
fn invalid_octal_number_message() {
    let (_, err, _) = rendered_pair("[%d]", &["08"]);
    assert_eq!(err, "printf: 08: invalid octal number\n");
}

#[test]
fn invalid_hex_number_message() {
    let (_, err, _) = rendered_pair("[%d]", &["0xg"]);
    assert_eq!(err, "printf: 0xg: invalid hex number\n");
}

#[test]
fn integer_overflow_clamps_and_reports() {
    let (out, err, failed) = rendered_pair("[%d]", &["99999999999999999999"]);
    assert_eq!(out, "[9223372036854775807]");
    assert_eq!(
        err,
        "printf: 99999999999999999999: Numerical result out of range\n"
    );
    assert!(failed);
}

#[test]
fn hex_and_binary_integer_parsing() {
    assert_eq!(rendered("%d %d", &["0x10", "0b101"]), "16 5");
}

#[test]
fn float_fixed_default_precision() {
    assert_eq!(rendered("%f", &["3.14159"]), "3.141590");
}

#[test]
fn float_precision_and_zero_flag() {
    assert_eq!(rendered("%08.2f", &["3.5"]), "00003.50");
}

#[test]
fn float_exponent_form() {
    assert_eq!(rendered("%e", &["150"]), "1.500000e+02");
}

#[test]
fn float_g_form_switches() {
    assert_eq!(rendered("%g %g", &["100000", "1.5e-5"]), "100000 1.5e-05");
}

#[test]
fn float_inf_nan() {
    assert_eq!(rendered("%f %f", &["inf", "nan"]), "inf nan");
}

#[test]
fn hex_float_form() {
    assert_eq!(rendered("%a", &["1.5"]), "0xcp-3");
}

#[test]
fn float_invalid_number_reported() {
    let (out, err, failed) = rendered_pair("[%f]", &["abc"]);
    assert_eq!(out, "[0.000000]");
    assert_eq!(err, "printf: abc: invalid number\n");
    assert!(failed);
}

#[test]
fn float_range_overflow_reports_inf() {
    let (out, err, failed) = rendered_pair("[%f]", &["1e400"]);
    assert_eq!(out, "[inf]");
    assert_eq!(err, "printf: 1e400: Numerical result out of range\n");
    assert!(failed);
}

#[test]
fn string_width_and_precision() {
    assert_eq!(rendered("[%8.3s]", &["hello"]), "[     hel]");
}

#[test]
fn string_left_justify() {
    assert_eq!(rendered("[%-8s|]", &["hi"]), "[hi      |]");
}

#[test]
fn int_width_precision_flags() {
    assert_eq!(
        rendered("%08.5d %5d %-5d %+d % d", &["42", "5", "5", "5", "5"]),
        "   00042     5 5     +5  5"
    );
}

#[test]
fn octal_hex_alt_flag() {
    assert_eq!(
        rendered("%#o %#x %#X", &["255", "255", "255"]),
        "0377 0xff 0XFF"
    );
}

#[test]
fn star_width_and_precision() {
    assert_eq!(rendered("%*d", &["5", "7"]), "    7");
    assert_eq!(rendered("%*.*f", &["8", "-1", "3.14159"]), "3.141590");
}

#[test]
fn star_width_from_hex_arg() {
    assert_eq!(rendered("%*d", &["0x10", "7"]), "               7");
}

#[test]
fn star_invalid_width_is_zero_and_reported() {
    let (out, err, failed) = rendered_pair("%*d", &["abc", "7"]);
    assert_eq!(out, "7");
    assert_eq!(err, "printf: abc: invalid number\n");
    assert!(failed);
}

#[test]
fn b_expands_argument_escapes() {
    assert_eq!(rendered("%b", &[r"a\nb"]), "a\nb");
}

#[test]
fn b_numeric_argument_verbatim() {
    assert_eq!(rendered("%b", &["12345"]), "12345");
}

#[test]
fn b_precision_truncates_before_expansion() {
    assert_eq!(rendered("%.2b", &[r"ab\ncd"]), "ab");
}

#[test]
fn q_empty_is_quoted() {
    assert_eq!(rendered("%q", &[""]), "''");
}

#[test]
fn q_plain_word_unquoted() {
    assert_eq!(rendered("%q", &["hello"]), "hello");
}

#[test]
fn q_metacharacters_escaped() {
    assert_eq!(rendered("%q", &["a b"]), "a\\ b");
}

#[test]
fn q_control_uses_dollar_form() {
    assert_eq!(rendered("%q", &["a\nb"]), "$'a\\nb'");
}
