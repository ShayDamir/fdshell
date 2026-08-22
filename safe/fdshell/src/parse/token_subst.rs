use crate::error::parse::{ParseError, report_unexpected_eof};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(crate) fn read_dollar_paren(
    line: &[u8],
    cur: &mut ShortCStr,
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    pos: &mut usize,
) -> Result<(), Report<ParseError>> {
    let start = *pos - 1;
    cur.push_byte(b'$')
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    cur.push_byte(b'(')
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    bytes.next(); // consume '('
    *pos += 1;
    let mut depth = 1u32;
    let mut in_quotes = false;
    loop {
        let Some(c) = bytes.next() else {
            return Err(report_unexpected_eof(line, start));
        };
        *pos += 1;
        // Inside double quotes a backslash makes the next byte data: it
        // neither toggles the quote nor counts as a paren.
        if in_quotes && c == b'\\' {
            let Some(escaped) = bytes.next() else {
                return Err(report_unexpected_eof(line, start));
            };
            *pos += 1;
            cur.push_byte(b'\\')
                .change_context(ParseError::InvalidChar { ch: 0 })?;
            cur.push_byte(escaped)
                .change_context(ParseError::InvalidChar { ch: 0 })?;
            continue;
        }
        cur.push_byte(c)
            .change_context(ParseError::InvalidChar { ch: 0 })?;
        match c {
            b'"' => in_quotes = !in_quotes,
            b'(' if !in_quotes => depth += 1,
            b')' if !in_quotes => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
    }
    Ok(())
}
