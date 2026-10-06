//! `a`/`A` conversion rendering: the `[8,16)` hex-float form on the f64 value.
//!
//! The mantissa carries at most 13 fractional hex digits (f64 precision); bash's
//! `%a` uses long double (16 digits), so exact digit-for-digit agreement with
//! bash is not expected — only the correctly-rounded f64 value is.

use alloc::vec::Vec;

use super::fmt_field::pad;
use super::fmt_float::{apply_sign, nonfinite};
use super::fmt_hexfloat_prec::{apply_precision, format_mantissa};
use super::spec::Fmt;

/// Render an `a`/`A` conversion (the value is already parsed) into `out`.
pub(super) fn render(v: f64, fmt: &Fmt, out: &mut Vec<u8>) {
    let upper = fmt.conv == b'A';
    if let Some(s) = nonfinite(v, upper) {
        pad(
            fmt.left,
            fmt.zero,
            fmt.width.unwrap_or(0),
            s.as_bytes(),
            out,
        );
        return;
    }
    if v == 0.0 {
        let s = if upper { "0X0P+0" } else { "0x0p+0" };
        pad(
            fmt.left,
            fmt.zero,
            fmt.width.unwrap_or(0),
            s.as_bytes(),
            out,
        );
        return;
    }
    let (h, exact, exp2) = mantissa(v);
    let (h, digits, exp2) = apply_precision(h, &exact, exp2, fmt.precision);
    let body = format_mantissa(h, &digits, exp2, upper);
    let s = apply_sign(body, v, fmt);
    pad(
        fmt.left,
        fmt.zero,
        fmt.width.unwrap_or(0),
        s.as_bytes(),
        out,
    );
}

/// The `[8,16)` significand of a nonzero finite `v`: the top nibble `h` (8..=15),
/// the 13 exact fractional hex digits, and the base-2 exponent `exp2` such that
/// `v = (h + fraction) × 2^exp2`.
fn mantissa(v: f64) -> (u8, [u8; 13], i32) {
    let bits = v.to_bits();
    let expb = ((bits >> 52) & 0x7FF) as i32;
    let frac = bits & 0x000F_FFFF_FFFF_FFFF;
    let (s, e) = if expb == 0 {
        let mut s = frac;
        let sh = 52 - (64 - s.leading_zeros()) + 1;
        s <<= sh;
        (s, -1074 - i32::try_from(sh).unwrap_or(i32::MAX))
    } else {
        ((1u64 << 52) + frac, expb - 1075)
    };
    let h = (s >> 49) as u8;
    let fracval = s & ((1u64 << 49) - 1);
    let full = (fracval << 3) & 0x000F_FFFF_FFFF_FFFF;
    let digs: [u8; 13] = core::array::from_fn(|i| ((full >> (4 * (12 - i))) & 0xF) as u8);
    (h, digs, e + 49)
}
