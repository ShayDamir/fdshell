//! Fractional-digit selection and formatting for the `a`/`A` hex-float form.

use alloc::string::String;
use alloc::vec::Vec;

/// Choose the printed fractional digits: with no precision, the exact digits
/// minus trailing zeros; with precision `p`, `p` digits rounded half-to-even.
pub(super) fn apply_precision(
    h: u8,
    exact: &[u8; 13],
    exp2: i32,
    prec: Option<usize>,
) -> (u8, Vec<u8>, i32) {
    match prec {
        None => {
            let mut n = 13;
            while n > 0 && exact.get(n - 1) == Some(&0) {
                n -= 1;
            }
            (h, exact.get(..n).unwrap_or(&[]).to_vec(), exp2)
        }
        Some(p) if p >= 13 => {
            let mut d = exact.to_vec();
            d.extend(core::iter::repeat_n(0, p - 13));
            (h, d, exp2)
        }
        Some(p) => round(h, exact, exp2, p),
    }
}

/// Round the exact digits to `p`, half-to-even. A mantissa that rounds up to 16
/// renormalizes to 8 with `exp2 + 1`.
fn round(h: u8, exact: &[u8; 13], exp2: i32, p: usize) -> (u8, Vec<u8>, i32) {
    let keep = exact.get(..p).unwrap_or(&[]);
    let dropped = exact.get(p..).unwrap_or(&[]);
    let d0 = dropped.first().copied().unwrap_or(0);
    let rest_nz = dropped.get(1..).is_some_and(|r| r.iter().any(|&d| d != 0));
    let last = keep.last().copied().unwrap_or(h);
    if !(d0 > 8 || (d0 == 8 && (rest_nz || last % 2 == 1))) {
        return (h, keep.to_vec(), exp2);
    }
    let mut fv = 0u64;
    for &d in keep {
        fv = fv * 16 + u64::from(d);
    }
    let carry = (fv >> (4 * p)) as u8;
    let newfrac = fv & ((1u64 << (4 * p)) - 1);
    let mut h = h + carry;
    let mut exp2 = exp2;
    if h == 16 {
        h = 8;
        exp2 += 1;
    }
    (h, expand(newfrac, p), exp2)
}

/// Expand `v` into `p` hex digits (most significant first).
fn expand(v: u64, p: usize) -> Vec<u8> {
    (0..p)
        .map(|i| ((v >> (4 * (p - 1 - i))) & 0xF) as u8)
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
