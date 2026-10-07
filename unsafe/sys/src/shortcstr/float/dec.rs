//! Decimal (`digits[.digits][eE±digits]`) parsing.

use alloc::string::String;

use crate::shortcstr::exp::parse_exp;

/// `digits[.digits][eE±digits]`; at least one digit is required.
///
/// The scanned byte span is a valid decimal float by construction, so the
/// final `f64::from_str` (core, correctly rounded) cannot fail — the `0.0`
/// fallback is unreachable.
pub(super) fn parse_decimal(rest: &[u8]) -> (f64, usize, bool) {
    let mut i = 0;
    let mut digits = 0usize;
    let mut nonzero = false;
    let mut take = |c: u8| -> Option<()> {
        if c.is_ascii_digit() {
            digits += 1;
            nonzero |= c != b'0';
            Some(())
        } else {
            None
        }
    };
    while let Some(&c) = rest.get(i) {
        if take(c).is_some() {
            i += 1;
        } else {
            break;
        }
    }
    if rest.get(i) == Some(&b'.') {
        i += 1;
        while let Some(&c) = rest.get(i) {
            if take(c).is_some() {
                i += 1;
            } else {
                break;
            }
        }
    }
    if digits == 0 {
        return (0.0, 0, false);
    }
    let (_, i) = parse_exp(rest, i, b'e', b'E');
    // Hand the scanned span to core's correctly-rounded decimal→f64
    // conversion (a `sig * 10^exp` multiply double-rounds past 15 digits).
    let span = rest.get(..i).unwrap_or(&[]);
    let mut text = String::from(core::str::from_utf8(span).unwrap_or(""));
    if text.starts_with('.') {
        // Core rejects a bare `.5`; the leading zero is not part of the span.
        text.insert(0, '0');
    }
    // The scanned span is a valid decimal float by construction, so the
    // `0.0` fallback is unreachable.
    let value = text.parse::<f64>().unwrap_or(0.0);
    let range = nonzero && (value.is_infinite() || value == 0.0);
    (value, i, range)
}
