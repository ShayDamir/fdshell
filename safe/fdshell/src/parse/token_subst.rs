use crate::error::parse::{ParseError, report_unexpected_eof};
use error_stack::Report;
use sys::ShortCStr;

pub(crate) fn read_dollar_paren(
    line: &[u8],
    cur: &mut ShortCStr,
    bytes: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
    start: usize,
) -> Result<(), Report<ParseError>> {
    cur.push(b'$').map_err(ParseError::from)?;
    cur.push(b'(').map_err(ParseError::from)?;
    bytes.next(); // consume '('
    let mut depth = 1u32;
    while depth > 0 {
        match bytes.next() {
            Some(b'(') => {
                cur.push(b'(').map_err(ParseError::from)?;
                depth += 1;
            }
            Some(b')') => {
                depth -= 1;
                if depth == 0 {
                    cur.push(b')').map_err(ParseError::from)?;
                    break;
                }
                cur.push(b')').map_err(ParseError::from)?;
            }
            Some(c) => cur.push(c).map_err(ParseError::from)?,
            None => return Err(report_unexpected_eof(line, start)),
        }
    }
    Ok(())
}

pub(crate) fn read_backtick(
    line: &[u8],
    cur: &mut ShortCStr,
    bytes: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
    start: usize,
) -> Result<(), Report<ParseError>> {
    cur.push(b'`').map_err(ParseError::from)?;
    loop {
        match bytes.next() {
            Some(b'`') => {
                cur.push(b'`').map_err(ParseError::from)?;
                return Ok(());
            }
            Some(b'\\') => match bytes.next() {
                Some(b'`') => cur.push(b'`').map_err(ParseError::from)?,
                Some(b'\\') => {
                    cur.push(b'\\').map_err(ParseError::from)?;
                    cur.push(b'\\').map_err(ParseError::from)?;
                }
                Some(c) => {
                    cur.push(b'\\').map_err(ParseError::from)?;
                    cur.push(c).map_err(ParseError::from)?;
                }
                None => return Err(report_unexpected_eof(line, start)),
            },
            Some(c) => cur.push(c).map_err(ParseError::from)?,
            None => return Err(report_unexpected_eof(line, start)),
        }
    }
}
