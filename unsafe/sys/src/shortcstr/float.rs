//! C `strtod`-style floating-point parsing with prefix semantics.
//!
//! Used by shell numerics (`printf`): optional sign, decimal
//! (`digits[.digits][eE±digits]`), hex-float (`0x[hex.][pP±digits]`), and
//! `inf`/`infinity`/`nan([payload])` in any case. Stops at the first
//! invalid byte; the caller compares `consumed` with the length to detect
//! trailing junk.
//!
//! The pieces: the whitespace/sign prologue in `sign`, the `inf`/`nan` words in
//! `inf_nan`, the decimal body in `dec`, the hex-float body in `hex`.

mod dec;
mod hex;
mod inf_nan;
mod sign;

use crate::shortcstr::ShortCStr;
use inf_nan::inf_nan;
use sign::{parse_sign, skip_space};

/// Outcome of [`ShortCStr::parse_float`].
///
/// `Debug`/`PartialEq` are unconditional: the `sys` integration tests
/// (`tests/shortcstr.rs`) assert on these values from a separate crate.
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
        let (i, neg) = parse_sign(bytes, skip_space(bytes));
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
