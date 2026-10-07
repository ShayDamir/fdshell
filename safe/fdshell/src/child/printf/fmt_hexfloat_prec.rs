//! Fractional-digit selection and formatting for the `a`/`A` hex-float form.

use alloc::string::String;
use alloc::vec::Vec;

/// Choose the printed fractional digits: with no precision, the exact digits
/// minus trailing zeros; with precision `p`, `p` digits rounded half-to-even.
pub(super) fn apply_precision(
    h: u8,
    frac60: u64,
    exp2: i32,
    prec: Option<usize>,
) -> (u8, Vec<u8>, i32) {
    match prec {
        None => {
            let digits = digits_of(frac60);
            let mut n = 15;
            while n > 0 && digits.get(n - 1) == Some(&0) {
                n -= 1;
            }
            (h, digits.get(..n).unwrap_or(&[]).to_vec(), exp2)
        }
        Some(p) if p >= 15 => {
            let mut d = digits_of(frac60).to_vec();
            d.extend(core::iter::repeat_n(0, p - 15));
            (h, d, exp2)
        }
        Some(p) => {
            let (h, frac, exp2) = round_frac(frac60, h, exp2, p);
            (h, expand(frac, p), exp2)
        }
    }
}

/// Round the 15-digit significand to `p` fractional digits, half-to-even. A
/// mantissa that rounds up to `0x10` renormalizes to `0x1` with `exp2 + 4`.
fn round_frac(frac60: u64, h: u8, exp2: i32, p: usize) -> (u8, u64, i32) {
    let shift = 60 - 4 * p;
    let unit = 1u128 << shift;
    let half = unit >> 1;
    let c = (h as u128) << 60 | frac60 as u128;
    let rem = c & (unit - 1);
    let mut kept = c & !(unit - 1);
    // A tie rounds so the last kept digit is even.
    if rem > half || (rem == half && (kept >> shift) & 1 == 1) {
        kept += unit;
    }
    let mut h = (kept >> 60) as u8;
    // The kept fraction's top `p` digits, aligned to the low positions.
    let frac = ((kept & ((1u128 << 60) - 1)) as u64) >> shift;
    let mut exp2 = exp2;
    if h == 16 {
        h = 1;
        exp2 += 4;
    }
    (h, frac, exp2)
}

/// The 15 fractional hex digits of `frac60` (most significant first).
fn digits_of(frac60: u64) -> [u8; 15] {
    core::array::from_fn(|i| ((frac60 >> (4 * (14 - i))) & 0xF) as u8)
}

/// Expand `frac` (the kept fraction) into `p` hex digits (most significant
/// first), zero-filling the high positions.
fn expand(frac: u64, p: usize) -> Vec<u8> {
    (0..p)
        .map(|i| ((frac >> (4 * (p - 1 - i))) & 0xF) as u8)
        .collect()
}

/// A hex digit (0-15) as a character, upper- or lower-case.
fn hexdigit(d: u8, upper: bool) -> Option<char> {
    let c = char::from_digit(u32::from(d), 16)?;
    Some(if upper { c.to_ascii_uppercase() } else { c })
}

/// Assemble `0xH.fracp±exp` (uppercase for `A`) from the significand parts.
pub(super) fn format_mantissa(h: u8, digits: &[u8], exp2: i32, upper: bool) -> String {
    let mut s = String::new();
    s.push_str(if upper { "0X" } else { "0x" });
    if let Some(c) = hexdigit(h, upper) {
        s.push(c);
    }
    if !digits.is_empty() {
        s.push('.');
        for &d in digits {
            if let Some(c) = hexdigit(d, upper) {
                s.push(c);
            }
        }
    }
    s.push(if upper { 'P' } else { 'p' });
    if exp2 >= 0 {
        s.push('+');
    }
    s.push_str(&alloc::format!("{exp2}"));
    s
}
