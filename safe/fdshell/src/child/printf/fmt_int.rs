//! Integer conversion rendering (`d i u o x X`).

use alloc::string::String;
use alloc::vec::Vec;

use super::fmt_field::pad;
use super::spec::Fmt;

/// Render an integer conversion (the value is already parsed) into `out`.
pub(super) fn render(value: i64, fmt: &Fmt, out: &mut Vec<u8>) {
    let (sign, digits) = body(value, fmt);
    let mut core = String::with_capacity(sign.len() + digits.len());
    core.push_str(sign);
    core.push_str(&digits);
    let zero = fmt.zero && fmt.precision.is_none();
    pad(fmt.left, zero, fmt.width.unwrap_or(0), core.as_bytes(), out);
}

/// The sign string and the precision-applied magnitude digits.
fn body(value: i64, fmt: &Fmt) -> (&'static str, String) {
    match fmt.conv {
        b'd' | b'i' => signed(value, fmt),
        b'u' => unsigned(value as u64, fmt),
        b'o' => octal(value as u64, fmt),
        b'x' => hex(value as u64, false, fmt),
        b'X' => hex(value as u64, true, fmt),
        // The dispatcher only calls this for integer conversions.
        _ => ("", String::new()),
    }
}

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

fn unsigned(value: u64, fmt: &Fmt) -> (&'static str, String) {
    let mut digits = alloc::format!("{value}");
    min_digits(&mut digits, value, fmt);
    ("", digits)
}

fn octal(value: u64, fmt: &Fmt) -> (&'static str, String) {
    let mut digits = alloc::format!("{value:o}");
    if fmt.alt && value != 0 {
        digits.insert(0, '0');
    }
    min_digits(&mut digits, value, fmt);
    ("", digits)
}

fn hex(value: u64, upper: bool, fmt: &Fmt) -> (&'static str, String) {
    let mut digits = if upper {
        alloc::format!("{value:X}")
    } else {
        alloc::format!("{value:x}")
    };
    min_digits(&mut digits, value, fmt);
    if fmt.alt && value != 0 {
        digits.insert_str(0, if upper { "0X" } else { "0x" });
    }
    ("", digits)
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
