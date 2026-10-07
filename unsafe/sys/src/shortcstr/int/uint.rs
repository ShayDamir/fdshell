//! Base-0 unsigned integer parsing (C `strtoull` semantics) for `u o x X`.

use super::scan::{digit, pick_base, skip_sign};
use crate::shortcstr::ShortCStr;

/// Outcome of [`ShortCStr::parse_base0_uint`].
///
/// `Debug`/`PartialEq` are unconditional: the `sys` integration tests
/// (`tests/shortcstr.rs`) assert on these values from a separate crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UintParse {
    /// The whole string is a valid number.
    Exact(u64),
    /// A valid prefix was parsed, followed by non-numeric bytes.
    Trailing(u64),
    /// No numeric prefix at all.
    Nothing,
    /// The magnitude overflowed `u64`; saturated to `u64::MAX`.
    Overflow(u64),
}

impl ShortCStr {
    /// Parse a base-0 unsigned integer with C `strtoull` prefix semantics.
    ///
    /// Unlike [`ShortCStr::parse_base0_int`], values in
    /// `(i64::MAX, u64::MAX]` are valid; a negative input wraps in two's
    /// complement, and a magnitude above `u64::MAX` saturates to `u64::MAX`
    /// (`Overflow`), matching glibc `strtoull`.
    pub fn parse_base0_uint(&self) -> UintParse {
        let bytes = match self.as_bytes() {
            Ok(b) => b,
            Err(_) => return UintParse::Nothing,
        };
        let (neg, i) = skip_sign(bytes);
        let rest = bytes.get(i..).unwrap_or(&[]);
        let (radix, start) = pick_base(rest);
        let (value, digits, end) = accumulate(rest, start, radix);
        if digits == 0 {
            return UintParse::Nothing;
        }
        let (v, overflow) = clamp(neg, value);
        match (overflow, end == rest.len()) {
            (true, _) => UintParse::Overflow(v),
            (false, true) => UintParse::Exact(v),
            (false, false) => UintParse::Trailing(v),
        }
    }
}

/// Accumulate the digit run: the unsigned magnitude, the digit count, and the
/// index just past the last digit.
fn accumulate(rest: &[u8], start: usize, radix: u32) -> (u128, usize, usize) {
    // The limit is `2^64` (one past `u64::MAX`): magnitudes up to it are
    // decidable, anything longer overflows `u64` regardless of the sign.
    let limit = (u64::MAX as u128) + 1;
    let mut value: u128 = 0;
    let mut digits = 0;
    let mut end = start;
    for &c in rest.get(start..).unwrap_or(&[]) {
        match digit(c, radix) {
            Some(d) => {
                value = value * radix as u128 + d as u128;
                digits += 1;
                end += 1;
                if value > limit {
                    // Any longer input overflows `u64` anyway.
                    break;
                }
            }
            None => break,
        }
    }
    (value, digits, end)
}

/// Map the magnitude to `u64`: saturate past `u64::MAX`, otherwise apply the
/// sign as a two's-complement wrap.
fn clamp(neg: bool, value: u128) -> (u64, bool) {
    if value > u64::MAX as u128 {
        (u64::MAX, true)
    } else if neg {
        // `value` is ≤ `2^64`, so the negation fits `i128`; the `as u64`
        // is the two's-complement wrap (e.g. `1` → `u64::MAX`).
        ((-(value as i128)) as u128 as u64, false)
    } else {
        (value as u64, false)
    }
}
