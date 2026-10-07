//! Numeric argument parsing for the `printf` conversions, with bash error
//! reporting. Failures report to the `Sink` and fall back to 0 / the clamp.

use core::ffi::CStr;
use sys::ShortCStr;

use super::super::Sink;
use super::next_arg_bytes;

/// The next argument parsed as a base-0 signed integer (for `d`/`i` and `*`
/// fields), with bash error reporting. A missing argument is `0` with no error.
pub(super) fn next_int(rest: &mut &[&CStr], sink: &mut Sink) -> i64 {
    let Some(a) = next_arg_bytes(rest) else {
        return 0;
    };
    match to_shortcstr(&a).parse_base0_int() {
        sys::IntParse::Exact(v) => v,
        sys::IntParse::Overflow(v) => {
            sink.report_num(&a, "Numerical result out of range");
            v
        }
        // Bash prints the parsed prefix value (not 0) on a trailing-junk error.
        sys::IntParse::Trailing(v) => {
            sink.report_num(&a, num_reason(&a));
            v
        }
        sys::IntParse::Nothing => {
            sink.report_num(&a, num_reason(&a));
            0
        }
    }
}

/// The next argument parsed as a base-0 unsigned integer (for `u`/`o`/`x`/`X`),
/// with bash error reporting. Values in `(i64::MAX, u64::MAX]` are valid; a
/// negative input wraps in two's complement. A missing argument is `0`.
pub(super) fn next_uint(rest: &mut &[&CStr], sink: &mut Sink) -> u64 {
    let Some(a) = next_arg_bytes(rest) else {
        return 0;
    };
    match to_shortcstr(&a).parse_base0_uint() {
        sys::UintParse::Exact(v) => v,
        sys::UintParse::Overflow(v) => {
            sink.report_num(&a, "Numerical result out of range");
            v
        }
        sys::UintParse::Trailing(v) => {
            sink.report_num(&a, num_reason(&a));
            v
        }
        sys::UintParse::Nothing => {
            sink.report_num(&a, num_reason(&a));
            0
        }
    }
}

/// The next argument parsed as a floating-point number, with bash error
/// reporting. A missing argument is `0.0` with no error.
pub(super) fn next_float(rest: &mut &[&CStr], sink: &mut Sink) -> f64 {
    let Some(a) = next_arg_bytes(rest) else {
        return 0.0;
    };
    let p = to_shortcstr(&a).parse_float();
    if p.consumed == 0 || p.consumed < a.len() {
        // Bash prints the parsed prefix (0.0 when nothing parsed) on a
        // trailing-junk error, like the integer conversions.
        sink.report_num(&a, "invalid number");
        p.value
    } else if p.range {
        sink.report_num(&a, "Numerical result out of range");
        p.value
    } else {
        p.value
    }
}

/// The bash numeric-error message, chosen from the raw argument bytes.
fn num_reason(bytes: &[u8]) -> &'static str {
    if bytes.first() == Some(&b'0') {
        match bytes.get(1) {
            Some(c) if c.is_ascii_digit() => "invalid octal number",
            Some(&b'x') => "invalid hex number",
            _ => "invalid number",
        }
    } else {
        "invalid number"
    }
}

/// Wrap `bytes` as a `ShortCStr`. The bytes are a `CStr` payload (NUL-free),
/// so `from_vec` is `Ok`; the empty fallback keeps this total without an
/// `unwrap` (the `Err` arm is unreachable).
fn to_shortcstr(bytes: &[u8]) -> ShortCStr {
    match ShortCStr::from_vec(bytes.to_vec()) {
        Ok(s) => s,
        Err(_) => ShortCStr::new(),
    }
}
