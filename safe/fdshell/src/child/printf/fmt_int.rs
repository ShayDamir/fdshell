//! Integer conversion rendering (`d i` signed, `u o x X` unsigned).

use alloc::string::String;
use alloc::vec::Vec;

use super::fmt_field::pad;
use super::spec::Fmt;

/// Render a signed integer conversion (`d`/`i`, the value already parsed).
pub(super) fn render_signed(value: i64, fmt: &Fmt, out: &mut Vec<u8>) {
    let (sign, digits) = signed(value, fmt);
    let zero = fmt.zero && fmt.precision.is_none();
    pad(
        fmt.left,
        zero,
        fmt.width.unwrap_or(0),
        sign.as_bytes(),
        digits.as_bytes(),
        out,
    );
}

/// Render an unsigned integer conversion (`u`/`o`/`x`/`X`, the value parsed as
/// a full `u64`).
pub(super) fn render_unsigned(value: u64, fmt: &Fmt, out: &mut Vec<u8>) {
    let digits = match fmt.conv {
        b'u' => unsigned(value, fmt),
        b'o' => octal(value, fmt),
        b'x' => hex(value, false, fmt),
        b'X' => hex(value, true, fmt),
        // The dispatcher only calls this for unsigned conversions.
        _ => String::new(),
    };
    let zero = fmt.zero && fmt.precision.is_none();
    pad(
        fmt.left,
        zero,
        fmt.width.unwrap_or(0),
        b"",
        digits.as_bytes(),
        out,
    );
}

/// The sign string and the precision-applied magnitude digits.
fn signed(value: i64, fmt: &Fmt) -> (&'static str, String) {
    let mag = value.unsigned_abs();
    let mut digits = alloc::format!("{mag}");
    min_digits(&mut digits, mag, fmt);
    let sign = if value < 0 {
        "-"
    } else if fmt.plus {
        "+"
    } else if fmt.space {
        " "
    } else {
        ""
    };
    (sign, digits)
}

fn unsigned(value: u64, fmt: &Fmt) -> String {
    let mut digits = alloc::format!("{value}");
    min_digits(&mut digits, value, fmt);
    digits
}

fn octal(value: u64, fmt: &Fmt) -> String {
    let mut digits = alloc::format!("{value:o}");
    if fmt.alt && value != 0 {
        digits.insert(0, '0');
    }
    min_digits(&mut digits, value, fmt);
    digits
}

fn hex(value: u64, upper: bool, fmt: &Fmt) -> String {
    let mut digits = if upper {
        alloc::format!("{value:X}")
    } else {
        alloc::format!("{value:x}")
    };
    min_digits(&mut digits, value, fmt);
    if fmt.alt && value != 0 {
        digits.insert_str(0, if upper { "0X" } else { "0x" });
    }
    digits
}

/// Apply the precision as a minimum digit count (zero-padded on the left).
/// A zero value with an explicit precision of `0` renders no digits.
fn min_digits(digits: &mut String, value: u64, fmt: &Fmt) {
    let Some(p) = fmt.precision else {
        return;
    };
    if value == 0 && p == 0 {
        digits.clear();
    } else if digits.len() < p {
        digits.insert_str(0, &"0".repeat(p - digits.len()));
    }
}
