//! The radix renderers of the integer conversions: `o` (octal, the `#` flag
//! prefixes a `0`) and `x`/`X` (hex, the `#` flag prefixes `0x`/`0X`, the case
//! of the prefix following the conversion letter). The precision rule is the
//! shared one, `super::min_digits`; the `0x` prefix is inserted **after** it,
//! so the zeros land between the prefix and the digits.

use alloc::string::String;

use super::Fmt;
use super::min_digits;

pub(super) fn octal(value: u64, fmt: &Fmt) -> String {
    let mut digits = alloc::format!("{value:o}");
    if fmt.alt && value != 0 {
        digits.insert(0, '0');
    }
    min_digits(&mut digits, value, fmt);
    digits
}

pub(super) fn hex(value: u64, upper: bool, fmt: &Fmt) -> String {
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
