// mutants: -C --lib -- -E 'test(/^numarg::/)'

/// bash's `read -t` parser: no whitespace, `+` but no `-`, an optional
/// fraction attached to or instead of the digits (`0.5`, `.5`, `1.`). Whole
/// milliseconds out; exact for fractions of a millisecond, deeper digits
/// truncate.
pub(crate) fn seconds_ms(text: &[u8]) -> Option<u64> {
    let body = text.strip_prefix(b"+").unwrap_or(text);
    let (whole, frac) = match body.iter().position(|&b| b == b'.') {
        Some(dot) => (body.get(..dot)?, body.get(dot + 1..)?),
        None => (body, b"" as &[u8]),
    };
    if whole.is_empty() && frac.is_empty() {
        return None;
    }
    let ms = digits128(whole)?
        .checked_mul(1000)?
        .checked_add(fraction_ms(frac)?)?;
    u64::try_from(ms).ok()
}

/// A decimal digit run, empty allowed, accumulated without overflow.
fn digits128(digits: &[u8]) -> Option<u128> {
    let mut value: u128 = 0;
    for &d in digits {
        let n = u128::from(d.checked_sub(b'0')?);
        if n > 9 {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(n)?;
    }
    Some(value)
}

/// The fraction's milliseconds: the first three digits are tenths, hundredths
/// and thousandths; any deeper digit must still be a digit, and truncates.
fn fraction_ms(frac: &[u8]) -> Option<u128> {
    const PLACE: [u128; 3] = [100, 10, 1];
    let mut ms: u128 = 0;
    for (i, &d) in frac.iter().enumerate() {
        let n = u128::from(d.checked_sub(b'0')?);
        if n > 9 {
            return None;
        }
        if let Some(&place) = PLACE.get(i) {
            ms += n * place;
        }
    }
    Some(ms)
}
