use crate::error::parse::{ParseError, report_unexpected_eof};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(crate) fn read_dollar_paren(
    line: &[u8],
    cur: &mut ShortCStr,
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    start: usize,
) -> Result<(), Report<ParseError>> {
    cur.push_byte(b'$')
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    cur.push_byte(b'(')
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    bytes.next(); // consume '('
    let mut depth = 1u32;
    loop {
        match bytes.next() {
            Some(b'(') => {
                cur.push_byte(b'(')
                    .change_context(ParseError::InvalidChar { ch: 0 })?;
                depth += 1;
            }
            Some(b')') => {
                depth -= 1;
                cur.push_byte(b')')
                    .change_context(ParseError::InvalidChar { ch: 0 })?;
                if depth == 0 {
                    break;
                }
            }
            Some(c) => cur
                .push_byte(c)
                .change_context(ParseError::InvalidChar { ch: 0 })?,
            None => return Err(report_unexpected_eof(line, start)),
        }
    }
    Ok(())
}
