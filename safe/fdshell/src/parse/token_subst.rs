use crate::error::parse::{ParseError, report_unexpected_eof};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(crate) fn read_dollar_paren(
    line: &[u8],
    cur: &mut ShortCStr,
    bytes: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
    start: usize,
) -> Result<(), Report<ParseError>> {
    cur.push(b'$')
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    cur.push(b'(')
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    bytes.next(); // consume '('
    let mut depth = 1u32;
    while depth > 0 {
        match bytes.next() {
            Some(b'(') => {
                cur.push(b'(')
                    .change_context(ParseError::InvalidChar { ch: 0 })?;
                depth += 1;
            }
            Some(b')') => {
                depth -= 1;
                if depth == 0 {
                    cur.push(b')')
                        .change_context(ParseError::InvalidChar { ch: 0 })?;
                    break;
                }
                cur.push(b')')
                    .change_context(ParseError::InvalidChar { ch: 0 })?;
            }
            Some(c) => cur
                .push(c)
                .change_context(ParseError::InvalidChar { ch: 0 })?,
            None => return Err(report_unexpected_eof(line, start)),
        }
    }
    Ok(())
}
