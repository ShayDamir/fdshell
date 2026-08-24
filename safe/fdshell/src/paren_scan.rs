use alloc::vec::Vec;

/// Scan the body of a `$( … )` expression from a byte stream positioned just
/// after the opening `(`, tracking double-quote and backslash state: a
/// backslash inside double quotes shields the next byte, and quotes and
/// parens inside double quotes are data. Returns the body without the
/// surrounding parens, or `None` if the input ends before the matching `)`.
pub(crate) fn scan_dollar_paren_body(
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    let mut depth = 1u32;
    let mut in_quotes = false;
    while let Some(c) = bytes.next() {
        if in_quotes && c == b'\\' {
            let escaped = bytes.next()?;
            body.push(b'\\');
            body.push(escaped);
            continue;
        }
        if c == b')' && !in_quotes && depth == 1 {
            return Some(body);
        }
        body.push(c);
        match c {
            b'"' => in_quotes = !in_quotes,
            b'(' if !in_quotes => depth += 1,
            b')' if !in_quotes => depth -= 1,
            _ => {}
        }
    }
    None
}
