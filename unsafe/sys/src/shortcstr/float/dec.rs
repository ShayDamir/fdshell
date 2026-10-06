//! Decimal (`digits[.digits][eE±digits]`) parsing and the shared exponent scan.

/// Parse an exponent (`low`/`up` [±] digits) starting at `i`. Returns the
/// value and the new index; `i` is unchanged when the marker has no digits.
pub(super) fn parse_exp(rest: &[u8], mut i: usize, low: u8, up: u8) -> (i32, usize) {
    let Some(&c) = rest.get(i) else {
        return (0, i);
    };
    if c != low && c != up {
        return (0, i);
    }
    let exp_start = i;
    i += 1;
    let neg = matches!(rest.get(i), Some(b'-'));
    if matches!(rest.get(i), Some(b'+' | b'-')) {
        i += 1;
    }
    let start = i;
    let mut exp = 0i32;
    while let Some(&d) = rest.get(i) {
        if d.is_ascii_digit() {
            exp = exp.saturating_mul(10).saturating_add((d - b'0') as i32);
            i += 1;
        } else {
            break;
        }
    }
    if i == start {
        return (0, exp_start);
    }
    (if neg { -exp } else { exp }, i)
}

/// `digits[.digits][eE±digits]`; at least one digit is required.
pub(super) fn parse_decimal(rest: &[u8]) -> (f64, usize, bool) {
    let mut i = 0;
    let mut sig: u64 = 0;
    let mut digits = 0;
    let mut take = |c: u8| -> Option<u64> {
        let d = c.is_ascii_digit().then(|| (c - b'0') as u64)?;
        if digits < 19 {
            sig = sig * 10 + d;
            digits += 1;
        }
        Some(d)
    };
    while let Some(&c) = rest.get(i) {
        if take(c).is_some() {
            i += 1;
        } else {
            break;
        }
    }
    let mut frac = 0usize;
    if rest.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while let Some(&c) = rest.get(i) {
            if take(c).is_some() {
                i += 1;
            } else {
                break;
            }
        }
        frac = i - start;
    }
    if digits == 0 {
        return (0.0, 0, false);
    }
    let (e_exp, i) = parse_exp(rest, i, b'e', b'E');
    let exp = e_exp - (frac as i32);
    // `sig` holds at most 19 significant digits; for ≤15 of them the
    // `as f64` is exact and the product is a single correct rounding.
    let value = (sig as f64) * 10f64.powi(exp);
    let range = sig != 0 && (value.is_infinite() || value == 0.0);
    (value, i, range)
}
