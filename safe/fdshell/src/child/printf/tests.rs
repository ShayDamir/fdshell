#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use sys::ShortCStr;

use super::super::Ctx;
use super::{Sink, handle_printf, render};
use crate::state::ShellState;

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

/// `handle_printf` only reads `ctx.refs`; the other `Ctx` fields are dummies.
/// The `CString`s, `CStr` refs and `ShellState` are built inline so they outlive
/// the `Ctx` for the whole test body.
#[test]
fn handle_printf_writes_and_returns_zero() {
    let cs = refs(&["%s=%d", "hi", "42"]);
    let strs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let state = ShellState::new();
    let ctx = Ctx::new(ShortCStr::new(), &strs, &[], &state);
    assert_eq!(handle_printf(&ctx).unwrap(), 0);
}

#[test]
fn handle_printf_numeric_error_sets_status_and_stderr() {
    let cs = refs(&["%d", "abc"]);
    let strs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let state = ShellState::new();
    let ctx = Ctx::new(ShortCStr::new(), &strs, &[], &state);
    assert_eq!(handle_printf(&ctx).unwrap(), 1);
}

#[test]
fn handle_printf_no_args_prints_default_newline() {
    let state = ShellState::new();
    let ctx = Ctx::new(ShortCStr::new(), &[], &[], &state);
    assert_eq!(handle_printf(&ctx).unwrap(), 0);
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

// --- Pinned conversion table (POSIX/XCU, values cross-checked against bash) ---

#[test]
fn zero_fill_never_covers_the_sign() {
    assert_eq!(rendered("%05d", &["-5"]), "-0005");
    assert_eq!(rendered("%+08d", &["5"]), "+0000005");
    assert_eq!(rendered("%010f", &["-3.5"]), "-03.500000");
    assert_eq!(rendered("%08.5d", &["42"]), "   00042");
    assert_eq!(rendered("% +05d", &["5"]), "+0005");
}

#[test]
fn integer_stops_at_the_first_invalid_byte() {
    let (out, err, failed) = rendered_pair("%d", &["42x"]);
    assert_eq!(out, "42");
    assert_eq!(err, "printf: 42x: invalid number\n");
    assert!(failed);
}

#[test]
fn integer_prefix_of_a_float() {
    let (out, _, failed) = rendered_pair("%d", &["1.5"]);
    assert_eq!(out, "1");
    assert!(failed);
    assert_eq!(rendered("%x", &["1.5"]), "1");
}

#[test]
fn float_rounds_half_to_even_on_the_binary_value() {
    assert_eq!(rendered("%.1g", &["0.95"]), "0.9");
    assert_eq!(rendered("%.2f", &["1.005"]), "1.00");
    assert_eq!(rendered("%.2f", &["2.675"]), "2.67");
    assert_eq!(rendered("%.1f", &["0.95"]), "0.9");
    assert_eq!(rendered("%f", &["0.1"]), "0.100000");
    assert_eq!(rendered("%.0f", &["0.5"]), "0");
    assert_eq!(rendered("%.1f", &["2.5"]), "2.5");
}

#[test]
fn hex_float_sign_prefix() {
    assert_eq!(rendered("%a", &["-1.5"]), "-0xcp-3");
    assert_eq!(rendered("%A", &["-1.5"]), "-0XCP-3");
    assert_eq!(rendered("%a", &["-0.0"]), "-0x0p+0");
    assert_eq!(rendered("%a", &["0.0"]), "0x0p+0");
}

#[test]
fn hex_float_precision_rounds_half_to_even() {
    assert_eq!(rendered("%.0a", &["0.1"]), "0xdp-7");
    assert_eq!(rendered("%.1a", &["0.1"]), "0xc.dp-7");
    assert_eq!(rendered("%.2a", &["0.1"]), "0xc.cdp-7");
    assert_eq!(rendered("%.1a", &["1.5"]), "0xc.0p-3");
    assert_eq!(rendered("%.1a", &["0.6"]), "0x9.ap-4");
    assert_eq!(rendered("%.0a", &["8.5"]), "0x8p+0");
    assert_eq!(rendered("%.0a", &["15.5"]), "0x1p+4");
    assert_eq!(rendered("%.0a", &["9.5"]), "0xap+0");
    assert_eq!(rendered("%.1a", &["8.5"]), "0x8.8p+0");
    assert_eq!(rendered("%.1A", &["0.1"]), "0XC.DP-7");
}

#[test]
fn hex_float_full_significand_is_fifteen_digits() {
    // Exactly-representable values agree with bash digit-for-digit.
    assert_eq!(rendered("%a", &["1.5"]), "0xcp-3");
    assert_eq!(rendered("%a", &["10"]), "0xap+0");
    assert_eq!(rendered("%a", &["0.5"]), "0x8p-4");
    assert_eq!(rendered("%a", &["3"]), "0xcp-2");
    assert_eq!(rendered("%a", &["1"]), "0x8p-3");
    // The 15-digit form is visible with an explicit precision: the f64
    // significand (53 bits = 13 hex digits) zero-extends to 15.
    assert_eq!(rendered("%.15a", &["0.1"]), "0xc.cccccccccccd000p-7");
    // Documented f64 deviation: inexact values carry the f64 trailing digits,
    // not bash's long-double digits (`0.1` → bash `0xc.ccccccccccccccdp-7`,
    // `1e300` → `0xb.f21e44003acdd2dp+993`, `5e-324` →
    // `0x8.18995ce7aa0e1b2p-1077`). The smallest subnormal must not underflow.
    assert_eq!(rendered("%a", &["0.1"]), "0xc.cccccccccccdp-7");
    assert_eq!(rendered("%a", &["1e300"]), "0xb.f21e44003acep+993");
    assert_eq!(rendered("%a", &["12345.678"]), "0xc.0e6b645a1cacp+10");
    assert_eq!(rendered("%a", &["5e-324"]), "0x8p-1077");
    // The largest subnormal (52-bit fraction set) has a non-trivial leading-zero
    // count, pinning the subnormal bit-length math.
    assert_eq!(
        rendered("%a", &["2.2250738585072009e-308"]),
        "0xf.ffffffffffffp-1026"
    );
}

#[test]
fn unsigned_int_accepts_the_full_u64_range() {
    assert_eq!(
        rendered("%u", &["18446744073709551615"]),
        "18446744073709551615"
    );
    assert_eq!(rendered("%u", &["-1"]), "18446744073709551615");
    let (out, err, failed) = rendered_pair("%u", &["18446744073709551616"]);
    assert_eq!(out, "18446744073709551615");
    assert_eq!(
        err,
        "printf: 18446744073709551616: Numerical result out of range\n"
    );
    assert!(failed);
    assert_eq!(
        rendered("%o", &["18446744073709551615"]),
        "1777777777777777777777"
    );
    assert_eq!(
        rendered("%x", &["18446744073709551615"]),
        "ffffffffffffffff"
    );
    assert_eq!(
        rendered("%X", &["18446744073709551615"]),
        "FFFFFFFFFFFFFFFF"
    );
}

#[test]
fn negative_star_width_left_justifies() {
    assert_eq!(rendered("%*d", &["-5", "42"]), "42   ");
    assert_eq!(rendered("%*s", &["-5", "hi"]), "hi   ");
    assert_eq!(rendered("%*c", &["-3", "ab"]), "a  ");
    assert_eq!(rendered("%*.2f", &["-8", "3.14159"]), "3.14    ");
}

#[test]
fn exponent_form_is_uppercase_for_capital_specifiers() {
    assert_eq!(rendered("%E", &["150"]), "1.500000E+02");
    assert_eq!(rendered("%.3E", &["150"]), "1.500E+02");
    assert_eq!(rendered("%E", &["0.00015"]), "1.500000E-04");
    assert_eq!(rendered("%G", &["1.5e-5"]), "1.5E-05");
    assert_eq!(rendered("%G", &["123456"]), "123456");
    assert_eq!(rendered("%.2G", &["123.456"]), "1.2E+02");
}

#[test]
fn width_precision_and_left_justify() {
    assert_eq!(rendered("%-5.2f", &["3.14159"]), "3.14 ");
    assert_eq!(rendered("%10.2f", &["3.14159"]), "      3.14");
    assert_eq!(rendered("%06.2f", &["3.14159"]), "003.14");
    assert_eq!(rendered("%10.2f", &["-3.14159"]), "     -3.14");
    assert_eq!(rendered("%#.0f", &["5"]), "5.");
    assert_eq!(rendered("%5s", &["hi"]), "   hi");
}

#[test]
fn int_precision_zero_is_empty() {
    assert_eq!(rendered("%.0d", &["0"]), "");
    assert_eq!(rendered("%.0d", &["5"]), "5");
    assert_eq!(rendered("%.5d", &["42"]), "00042");
    assert_eq!(rendered("%.3o", &["8"]), "010");
}

#[test]
fn hex_float_input_with_a_f_digits() {
    // `0x1.afp2` = 6.734375; the `a`/`f` arms of the hex-digit table are load-bearing.
    assert_eq!(rendered("%f", &["0x1.afp2"]), "6.734375");
    assert_eq!(rendered("%f", &["0x1.AFP2"]), "6.734375");
    assert_eq!(rendered("%a", &["0x1.afp2"]), "0xd.78p-1");
}

#[test]
fn float_20_digit_significand_is_correctly_rounded() {
    // The parser is correctly rounded (a `sig * 10^exp` multiply would not be).
    assert_eq!(rendered("%.2f", &["0.30000000000000004"]), "0.30");
    // 17-digit integer: fdshell prints the correctly-rounded `f64` (`...568`,
    // the C/POSIX f64 behavior); bash preserves the decimal input (`...567`) —
    // a documented f64-scope deviation.
    assert_eq!(
        rendered("%f", &["12345678901234567.0"]),
        "12345678901234568.000000"
    );
}

#[test]
fn q_metacharacter_table() {
    assert_eq!(rendered("%q", &["a+b"]), "a+b");
    assert_eq!(rendered("%q", &["a;b"]), "a\\;b");
    assert_eq!(rendered("%q", &["a$b"]), "a\\$b");
    assert_eq!(rendered("%q", &["a b c"]), "a\\ b\\ c");
    assert_eq!(rendered("%q", &["a=b"]), "a=b");
}

#[test]
fn q_hash_tilde_only_at_word_start() {
    assert_eq!(rendered("%q", &["#a"]), "\\#a");
    assert_eq!(rendered("%q", &["a#b"]), "a#b");
    assert_eq!(rendered("%q", &["~a"]), "\\~a");
    assert_eq!(rendered("%q", &["a~b"]), "a~b");
}

#[test]
fn q_control_uses_dollar_form_with_named_escape() {
    assert_eq!(rendered("%q", &["a\tb"]), "$'a\\tb'");
    assert_eq!(rendered("%q", &["a\nb"]), "$'a\\nb'");
}

#[test]
fn q_dollar_form_named_escapes() {
    assert_eq!(rendered("%q", &["\u{7}"]), "$'\\a'");
    assert_eq!(rendered("%q", &["\u{8}"]), "$'\\b'");
    assert_eq!(rendered("%q", &["\u{b}"]), "$'\\v'");
    assert_eq!(rendered("%q", &["\u{c}"]), "$'\\f'");
    assert_eq!(rendered("%q", &["\u{d}"]), "$'\\r'");
    assert_eq!(rendered("%q", &["\u{1b}"]), "$'\\E'");
}

#[test]
fn q_dollar_form_backslash_and_quote_are_named() {
    // A control char forces the dollar form; the backslash and quote keep their
    // named escapes inside it.
    assert_eq!(rendered("%q", &["a\n\\"]), "$'a\\n\\\\'");
    assert_eq!(rendered("%q", &["a\n'"]), "$'a\\n\\''");
}

#[test]
fn q_dollar_form_octal_for_unnamed_control() {
    assert_eq!(rendered("%q", &["\u{1}"]), "$'\\001'");
}

#[test]
fn q_dollar_form_keeps_valid_utf8_raw() {
    assert_eq!(rendered("%q", &["héllo\n"]), "$'héllo\\n'");
}

#[test]
fn hex_float_inf_nan() {
    assert_eq!(rendered("%a", &["inf"]), "inf");
    assert_eq!(rendered("%a", &["nan"]), "nan");
    assert_eq!(rendered("%a", &["-inf"]), "-inf");
    assert_eq!(rendered("%A", &["inf"]), "INF");
    assert_eq!(rendered("%A", &["nan"]), "NAN");
}

#[test]
fn g_inf_nan_and_edges() {
    assert_eq!(rendered("%g", &["inf"]), "inf");
    assert_eq!(rendered("%g", &["nan"]), "nan");
    assert_eq!(rendered("%g", &["-inf"]), "-inf");
    assert_eq!(rendered("%.0g", &["1.5"]), "2");
    assert_eq!(rendered("%g", &["0"]), "0");
}

#[test]
fn round_percent_percent_advances_past_both() {
    // `%%` advances by 2; a desynced scan (`i * 2`) would misread the tail.
    assert_eq!(rendered("x%%", &[]), "x%");
}

#[test]
fn star_precision_zero_rounds_to_integer() {
    // A `*` precision of 0 is a real precision (not "absent").
    assert_eq!(rendered("%.*f", &["0", "1.5"]), "2");
}

#[test]
fn q_invalid_utf8_byte_forces_dollar_octal() {
    // An invalid UTF-8 byte is a `Unit::Byte`: it forces the `$'…'` form and is
    // rendered as a three-digit octal escape.
    assert_eq!(
        super::conv_q::render(&[b'a', 0xff, b'b']),
        b"$'a\\377b'".to_vec()
    );
}

#[test]
fn f_zero_fill_nonfinite_splits_sign() {
    // The leading `-` stays out of the zero-fill; `inf`/`-inf` carry it.
    assert_eq!(rendered("%010f", &["inf"]), "0000000inf");
    assert_eq!(rendered("%010f", &["-inf"]), "-000000inf");
}

#[test]
fn exponent_zero_is_positive() {
    // A zero exponent is `+00`, not `-00`.
    assert_eq!(rendered("%e", &["1.0"]), "1.000000e+00");
}

#[test]
fn g_zero_fill_nonfinite_splits_sign() {
    assert_eq!(rendered("%010g", &["inf"]), "0000000inf");
    assert_eq!(rendered("%010g", &["-inf"]), "-000000inf");
}

#[test]
fn a_zero_fill_nonfinite_splits_sign() {
    assert_eq!(rendered("%010a", &["inf"]), "0000000inf");
    assert_eq!(rendered("%010a", &["-inf"]), "-000000inf");
}

#[test]
fn hex_float_precision_above_15_zero_extends() {
    // `p > 15` zero-extends the 15-digit f64 significand (trailing digits are
    // the f64 form, per the documented deviation).
    assert_eq!(rendered("%.20a", &["0.1"]), "0xc.cccccccccccd00000000p-7");
}

#[test]
fn unsigned_width_without_zero_flag_space_fills() {
    // A width without the `0` flag space-fills (never zero-fills).
    assert_eq!(rendered("%5u", &["5"]), "    5");
}

#[test]
fn float_trailing_garbage_is_an_error() {
    // A partial float parse (`1.5x`) is a numeric error: the parsed prefix is
    // printed (like the integer conversions) and the exit status is set.
    let (out, _err, failed) = rendered_pair("%f", &["1.5x"]);
    assert_eq!(out, "1.500000");
    assert!(failed);
}

#[test]
fn flag_after_width_is_an_invalid_format() {
    // Flags precede the width; a flag after a width is not a valid spec.
    let _ = render_err("%5+d", &[]);
}
