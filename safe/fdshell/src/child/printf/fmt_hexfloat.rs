//! `a`/`A` conversion rendering: the glibc `[8,16)` hex-float form.
//!
//! The value is f64-based, so the mantissa carries at most 15 fractional hex
//! digits (the f64 significand zero-extended); for inputs whose decimal→binary
//! mapping differs between `f64` and glibc's long double (e.g. `0.1`), the
//! trailing digits reflect the f64 value, not bash's long-double digits.

use alloc::vec::Vec;

use super::fmt_field::pad;
use super::fmt_float::{nonfinite, sign_prefix};
use super::fmt_hexfloat_prec::{apply_precision, format_mantissa};
use super::spec::Fmt;

/// Render an `a`/`A` conversion (the value is already parsed) into `out`.
pub(super) fn render(v: f64, fmt: &Fmt, out: &mut Vec<u8>) {
    let upper = fmt.conv == b'A';
    if let Some(word) = nonfinite(v, upper) {
        let (prefix, core) = split_sign(&word);
        pad(
            fmt.left,
            fmt.zero,
            fmt.width.unwrap_or(0),
            prefix,
            core,
            out,
        );
        return;
    }
    if v == 0.0 {
        // `-0.0` keeps its sign (glibc prints `-0x0p+0`).
        let body = if upper { "0X0P+0" } else { "0x0p+0" };
        let prefix = sign_prefix(v, fmt);
        pad(
            fmt.left,
            fmt.zero,
            fmt.width.unwrap_or(0),
            prefix.as_bytes(),
            body.as_bytes(),
            out,
        );
        return;
    }
    let (h, frac60, exp2) = mantissa(v);
    let (h, digits, exp2) = apply_precision(h, frac60, exp2, fmt.precision);
    let body = format_mantissa(h, &digits, exp2, upper);
    let prefix = sign_prefix(v, fmt);
    pad(
        fmt.left,
        fmt.zero,
        fmt.width.unwrap_or(0),
        prefix.as_bytes(),
        body.as_bytes(),
        out,
    );
}

/// The `[8,16)` significand of a nonzero finite `v`: the top nibble `h`
/// (8..=15), the 60-bit (15-digit) fractional significand, and the base-2
/// exponent `exp2` such that `v = (h + fraction) × 2^exp2`.
fn mantissa(v: f64) -> (u8, u64, i32) {
    let bits = v.to_bits();
    let expb = ((bits >> 52) & 0x7FF) as i32;
    let frac = bits & 0x000F_FFFF_FFFF_FFFF;
    // The significand `s` (its bit length `b`) and the base-2 exponent `e`
    // such that `v = s × 2^e`.
    let (s, b, e) = if expb == 0 {
        (frac, 64 - frac.leading_zeros(), -1074)
    } else {
        ((1u64 << 52) | frac, 53, expb - 1075)
    };
    // Left-justify `s` so its top nibble lands in bits 60..63 (value in [8,16)).
    let m = s << (64 - b);
    let h = (m >> 60) as u8;
    let frac60 = m & 0x0FFF_FFFF_FFFF_FFFF;
    (h, frac60, e + b as i32 - 4)
}

/// Split a leading `-` off a rendered word into `(sign, rest)`.
fn split_sign(word: &str) -> (&[u8], &[u8]) {
    let b = word.as_bytes();
    match b.split_first() {
        Some((c, rest)) if *c == b'-' => (core::slice::from_ref(c), rest),
        _ => (&[], b),
    }
}
