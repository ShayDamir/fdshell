use crate::error::parse::{ParseError, report_unexpected_eof};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(crate) fn read_backtick(
    line: &[u8],
    cur: &mut ShortCStr,
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    start: usize,
) -> Result<(), Report<ParseError>> {
    cur.push(b'`')
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    loop {
        match bytes.next() {
            Some(b'`') => {
                cur.push(b'`')
                    .change_context(ParseError::InvalidChar { ch: 0 })?;
                return Ok(());
            }
            Some(b'\\') => match bytes.next() {
                Some(b'`') => cur
                    .push(b'`')
                    .change_context(ParseError::InvalidChar { ch: 0 })?,
                Some(b'\\') => {
                    cur.push(b'\\')
                        .change_context(ParseError::InvalidChar { ch: 0 })?;
                    cur.push(b'\\')
                        .change_context(ParseError::InvalidChar { ch: 0 })?;
                }
                Some(c) => {
                    cur.push(b'\\')
                        .change_context(ParseError::InvalidChar { ch: 0 })?;
                    cur.push(c)
                        .change_context(ParseError::InvalidChar { ch: 0 })?;
                }
                None => return Err(report_unexpected_eof(line, start)),
            },
            Some(c) => cur
                .push(c)
                .change_context(ParseError::InvalidChar { ch: 0 })?,
            None => return Err(report_unexpected_eof(line, start)),
        }
    }
}
