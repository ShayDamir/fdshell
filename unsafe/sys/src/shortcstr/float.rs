//! C `strtod`-style floating-point parsing with prefix semantics.
//!
//! Used by shell numerics (`printf`): optional sign, decimal
//! (`digits[.digits][eE±digits]`), hex-float (`0x[hex.][pP±digits]`), and
//! `inf`/`infinity`/`nan([payload])` in any case. Stops at the first
//! invalid byte; the caller compares `consumed` with the length to detect
//! trailing junk.

mod dec;
mod hex;

use crate::shortcstr::ShortCStr;

/// Outcome of [`ShortCStr::parse_float`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatParse {
    /// The parsed value (`inf`/`0.0` on range overflow/underflow).
    pub value: f64,
    /// Bytes consumed (0 when nothing was parsed).
    pub consumed: usize,
    /// The `f64` result overflowed or underflowed the input range.
    pub range: bool,
}

impl ShortCStr {
    /// Parse a floating-point number with C `strtod` prefix semantics.
    ///
    /// Leading whitespace and a `+`/`-` sign are consumed; `0x`/`0X`
    /// selects hex-float. `inf`/`infinity` and `nan` (optionally
    /// `nan(payload)`) are recognized in any case.
    pub fn parse_float(&self) -> FloatParse {
        let bytes = match self.as_bytes() {
            Ok(b) => b,
            Err(_) => return FloatParse::none(),
        };
        let mut i = skip_space(bytes);
        let neg = match bytes.get(i) {
            Some(b'-') => {
                i += 1;
                true
            }
            Some(b'+') => {
                i += 1;
                false
            }
            _ => false,
        };
        let rest = bytes.get(i..).unwrap_or(&[]);
        let (value, consumed, range) = parse_body(rest);
        if consumed == 0 {
            // A lone sign is not part of a number.
            return FloatParse::none();
        }
        let value = if neg && value.is_sign_positive() {
            -value
        } else {
            value
        };
        FloatParse {
            value,
            consumed: i + consumed,
            range,
        }
    }
}

impl FloatParse {
    const fn none() -> Self {
        Self {
            value: 0.0,
            consumed: 0,
            range: false,
        }
    }
}

/// Parse the mantissa/exponent body after the sign.
fn parse_body(rest: &[u8]) -> (f64, usize, bool) {
    if let Some((v, n)) = inf_nan(rest) {
        return (v, n, false);
    }
    if rest.starts_with(b"0x") || rest.starts_with(b"0X") {
        return hex::parse_hex(rest);
    }
    dec::parse_decimal(rest)
}

/// `inf`/`infinity` and `nan([payload])`, any case; `None` otherwise.
fn inf_nan(rest: &[u8]) -> Option<(f64, usize)> {
    let ci = |s: &[u8], t: &[u8]| {
        s.len() >= t.len() && s.get(..t.len()).is_some_and(|p| p.eq_ignore_ascii_case(t))
    };
    if ci(rest, b"inf") {
        // "inf" (3) or "infinity" (8): the extra 5 bytes are present or not.
        let n = 3 + usize::from(ci(rest, b"infinity")) * 5;
        return Some((f64::INFINITY, n));
    }
    if ci(rest, b"nan") {
        let n = rest
            .get(3..)
            .filter(|s| s.first() == Some(&b'('))
            .and_then(|s| s.iter().position(|&c| c == b')').map(|close| 4 + close))
            .unwrap_or(3);
        return Some((f64::NAN, n));
    }
    None
}

fn skip_space(bytes: &[u8]) -> usize {
    let mut i = 0;
    while let Some(&c) = bytes.get(i) {
        if !matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
            break;
        }
        i += 1;
    }
    i
}
