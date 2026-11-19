// mutants: -C --lib -- -E 'test(/^numarg::/)'

/// One of bash's numeric-argument parsers.
pub(crate) struct Grammar {
    /// Whether surrounding whitespace is skipped (`strtoimax`-based grammars
    /// skip `isspace()` bytes at both ends).
    skip_whitespace: bool,
    /// Whether a leading `+`/`-` sign is accepted.
    allow_sign: bool,
}

/// bash's `legal_number` (base-10 `strtoimax`): whitespace skipped, sign
/// accepted, decimal. Users: `shift`, `read -n`, `exit`.
pub(crate) const LEGAL_NUMBER: Grammar = Grammar {
    skip_whitespace: true,
    allow_sign: true,
};

/// bash's strict digit-run parser: no whitespace, no sign, leading zeros stay
/// decimal. User: `ulimit`.
pub(crate) const ULIMIT_VALUE: Grammar = Grammar {
    skip_whitespace: false,
    allow_sign: false,
};

/// The parsed value under `grammar`, or `None` when the text is not in it.
/// Values past `i128` are `None`; narrower per-site ranges are the site's own
/// decision.
pub(crate) fn decimal(text: &[u8], grammar: &Grammar) -> Option<i128> {
    let trimmed = if grammar.skip_whitespace {
        trim(text)?
    } else {
        text
    };
    let (negative, digits) = split_sign(trimmed, grammar.allow_sign);
    let magnitude = accumulate(digits)?;
    if negative {
        magnitude.checked_neg()
    } else {
        Some(magnitude)
    }
}

/// The whole string trimmed when `grammar` skips whitespace; the same slice
/// otherwise (callers never trim).
fn trim(text: &[u8]) -> Option<&[u8]> {
    let is_ws = |b: &u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c);
    let start = text.iter().position(|b| !is_ws(b)).unwrap_or(0);
    let kept = text.len()
        - text
            .iter()
            .rev()
            .position(|b| !is_ws(b))
            .unwrap_or(text.len());
    text.get(start..kept)
}

/// The leading sign, peeled only when the grammar allows one.
fn split_sign(text: &[u8], allow_sign: bool) -> (bool, &[u8]) {
    if !allow_sign {
        return (false, text);
    }
    match text.first() {
        Some(b'+') => (false, text.get(1..).unwrap_or(&[])),
        Some(b'-') => (true, text.get(1..).unwrap_or(&[])),
        _ => (false, text),
    }
}

/// A decimal digit run, non-empty, accumulated into `i128` without overflow.
fn accumulate(digits: &[u8]) -> Option<i128> {
    let mut value: i128 = 0;
    let mut saw_digit = false;
    for &d in digits {
        let n = i128::from(d.checked_sub(b'0')?);
        if n > 9 {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(n)?;
        saw_digit = true;
    }
    saw_digit.then_some(value)
}
