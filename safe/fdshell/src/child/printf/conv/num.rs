//! Numeric argument parsing for the `printf` conversions, with bash error
//! reporting. Failures report to the `Sink` and fall back to 0 / the clamp.

use core::ffi::CStr;
use sys::ShortCStr;

use super::super::Sink;
use super::next_arg_bytes;

/// The next argument parsed as a base-0 integer, with bash error reporting. A
/// missing argument is `0` with no error.
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
        sys::IntParse::Trailing(_) | sys::IntParse::Nothing => {
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
        sink.report_num(&a, "invalid number");
        0.0
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

fn to_shortcstr(bytes: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(bytes.to_vec()).unwrap_or_default()
}
