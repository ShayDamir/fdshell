//! Hex-float (`0x[hex.][pP±digits]`) parsing.

use super::dec::parse_exp;

/// `0x[hex.][pP±digits]`; at least one hex digit is required.
pub(super) fn parse_hex(rest: &[u8]) -> (f64, usize, bool) {
    let mut i = 2;
    let mut sig: u64 = 0;
    let mut digits: u32 = 0;
    let mut frac = 0u32;
    while let Some(&c) = rest.get(i) {
        if take_hex(c, &mut sig, &mut digits).is_some() {
            i += 1;
        } else {
            break;
        }
    }
    if rest.get(i) == Some(&b'.') {
        i += 1;
        while let Some(&c) = rest.get(i) {
            if take_hex(c, &mut sig, &mut digits).is_some() {
                frac += 1;
                i += 1;
            } else {
                break;
            }
        }
    }
    if digits == 0 {
        return (0.0, 2, false);
    }
    let (p_exp, i) = parse_exp(rest, i, b'p', b'P');
    let exp = p_exp - (frac as i32) * 4;
    // Powers of two are exact, so the single `sig as f64` rounding is the
    // correctly-rounded result.
    let value = (sig as f64) * 2f64.powi(exp);
    let range = sig != 0 && (value.is_infinite() || value == 0.0);
    (value, i, range)
}

/// Accumulate one hex digit into `sig` (up to 16 digits); `None` when
/// `c` is not a hex digit.
fn take_hex(c: u8, sig: &mut u64, digits: &mut u32) -> Option<u64> {
    let d = hex_digit(c)?;
    if *digits < 16 {
        *sig = (*sig << 4) | d;
        *digits += 1;
    }
    Some(d)
}

fn hex_digit(c: u8) -> Option<u64> {
    match c {
        b'0'..=b'9' => Some(c as u64 - b'0' as u64),
        b'a'..=b'f' => Some(c as u64 - b'a' as u64 + 10),
        b'A'..=b'F' => Some(c as u64 - b'A' as u64 + 10),
        _ => None,
    }
}
