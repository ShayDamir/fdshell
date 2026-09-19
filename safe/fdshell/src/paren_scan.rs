use alloc::vec::Vec;

/// Scan the body of a parenthesized expression from a byte stream positioned
/// just after an opening `(`, tracking quote state: a backslash inside double
/// quotes shields the next byte, and quotes and parens inside double quotes
/// are data. `depth` counts the parens the caller already consumed, so the
/// body ends at the first `)` bringing the count back to `depth`. Returns the
/// body without that final `)`, or `None` if the input ends first.
pub(crate) fn scan_paren_body(
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    depth: u32,
) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    let mut d = depth;
    let mut in_quotes = false;
    while let Some(c) = bytes.next() {
        if in_quotes && c == b'\\' {
            let escaped = bytes.next()?;
            body.push(b'\\');
            body.push(escaped);
            continue;
        }
        if c == b')' && !in_quotes && d == depth {
            return Some(body);
        }
        body.push(c);
        match c {
            b'"' => in_quotes = !in_quotes,
            b'(' if !in_quotes => d += 1,
            b')' if !in_quotes => d -= 1,
            _ => {}
        }
    }
    None
}

/// Scan the body of a `$(…)` expression: one `(` consumed by the caller.
pub(crate) fn scan_dollar_paren_body(
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
) -> Option<Vec<u8>> {
    scan_paren_body(bytes, 1)
}
