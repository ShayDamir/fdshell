//! Base-0 integer parsing with C `strtol` prefix semantics.
//!
//! Used by shell numerics (`printf`, `*` field arguments): optional sign,
//! `0x`/`0X` hex, `0b`/`0B` binary, leading-`0` octal, else decimal. Stops
//! at the first invalid digit; overflow clamps to `i64::MAX`/`i64::MIN`.

mod scan;
pub(crate) mod uint;

use crate::shortcstr::ShortCStr;

/// Outcome of [`ShortCStr::parse_base0_int`].
///
/// `Debug`/`PartialEq` are unconditional: the `sys` integration tests
/// (`tests/shortcstr.rs`) assert on these values from a separate crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntParse {
    /// The whole string is a valid number.
    Exact(i64),
    /// A valid prefix was parsed, followed by non-numeric bytes.
    Trailing(i64),
    /// No numeric prefix at all.
    Nothing,
    /// Parsed to the end but overflowed `i64`; clamped to the bound.
    Overflow(i64),
}

impl ShortCStr {
    /// Parse a base-0 integer with C `strtol` prefix semantics.
    ///
    /// Leading whitespace and a `+`/`-` sign are consumed; `0x`/`0X` selects
    /// hex, `0b`/`0B` binary, a leading `0` octal, otherwise decimal.
    /// Parsing stops at the first invalid digit and reports how much was
    /// consumed; overflow clamps to `i64::MAX`/`i64::MIN`.
    pub fn parse_base0_int(&self) -> IntParse {
        let bytes = match self.as_bytes() {
            Ok(b) => b,
            Err(_) => return IntParse::Nothing,
        };
        let (neg, i) = scan::skip_sign(bytes);
        let rest = bytes.get(i..).unwrap_or(&[]);
        let (radix, start) = scan::pick_base(rest);
        let (value, digits, end) = accumulate(rest, start, radix);
        if digits == 0 {
            return IntParse::Nothing;
        }
        let (v, overflow) = clamp(neg, value);
        match (overflow, end == rest.len()) {
            (true, _) => IntParse::Overflow(v),
            (false, true) => IntParse::Exact(v),
            (false, false) => IntParse::Trailing(v),
        }
    }
}

/// The largest magnitude `i64` carries: `2^63`, the magnitude of `i64::MIN`.
/// Spelled from data (a method call plus a cast, no `+ 1`): the `+` of
/// `(i64::MAX as u128) + 1` mutated to `*` is the bound **itself**, a cap one
/// lower that stops the digit loop one digit early and reclassifies an
/// overflow as `Trailing` — and no operator means no mutant to chase.
const MAGNITUDE_CAP: u128 = i64::MIN.unsigned_abs() as u128;

/// Accumulate the digit run starting at `start`: the value, the digit count,
/// and the index just past the last digit.
fn accumulate(rest: &[u8], start: usize, radix: u32) -> (u128, usize, usize) {
    let mut value: u128 = 0;
    let mut digits = 0;
    let mut end = start;
    for &c in rest.get(start..).unwrap_or(&[]) {
        match scan::digit(c, radix) {
            Some(d) => {
                value = value * radix as u128 + d as u128;
                digits += 1;
                end += 1;
                if value > MAGNITUDE_CAP {
                    // Any longer input overflows `i64` at either sign, and the
                    // `end` past a digit is consulted only for the non-overflow
                    // `Exact`/`Trailing` split, so stopping here is sound.
                    break;
                }
            }
            None => break,
        }
    }
    (value, digits, end)
}

/// Clamp the unsigned magnitude to `i64`, honouring the sign.
fn clamp(neg: bool, value: u128) -> (i64, bool) {
    if neg {
        if value > MAGNITUDE_CAP {
            (i64::MIN, true)
        } else if value == MAGNITUDE_CAP {
            (i64::MIN, false)
        } else {
            // `value` is < 2^63 here, so the cast is exact.
            (-(value as i64), false)
        }
    } else if value > i64::MAX as u128 {
        (i64::MAX, true)
    } else {
        (value as i64, false)
    }
}
