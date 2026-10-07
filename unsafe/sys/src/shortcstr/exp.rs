//! Shared exponent scan for the float parsers (`eE` decimal, `pP` hex).

/// Parse an exponent (`low`/`up` [±] digits) starting at `i`. Returns the
/// value and the new index; `i` is unchanged when the marker has no digits.
pub(crate) fn parse_exp(rest: &[u8], mut i: usize, low: u8, up: u8) -> (i32, usize) {
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
