use alloc::vec::Vec;

use crate::error::parse::{ParseError, report_unexpected_eof};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(crate) fn read_backtick(
    line: &[u8],
    cur: &mut ShortCStr,
    mask: &mut Vec<bool>,
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    pos: &mut usize,
) -> Result<(), Report<ParseError>> {
    let start = *pos - 1;
    cur.push_byte(b'`')
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    mask.push(false);
    loop {
        match bytes.next() {
            Some(b'`') => {
                *pos += 1;
                cur.push_byte(b'`')
                    .change_context(ParseError::InvalidChar { ch: 0 })?;
                mask.push(false);
                return Ok(());
            }
            Some(b'\\') => {
                *pos += 1;
                match bytes.next() {
                    Some(b'`') => {
                        *pos += 1;
                        cur.push_byte(b'`')
                            .change_context(ParseError::InvalidChar { ch: 0 })?;
                        mask.push(false);
                    }
                    Some(b'\\') => {
                        *pos += 1;
                        cur.push_byte(b'\\')
                            .change_context(ParseError::InvalidChar { ch: 0 })?;
                        mask.push(false);
                        cur.push_byte(b'\\')
                            .change_context(ParseError::InvalidChar { ch: 0 })?;
                        mask.push(false);
                    }
                    Some(c) => {
                        *pos += 1;
                        cur.push_byte(b'\\')
                            .change_context(ParseError::InvalidChar { ch: 0 })?;
                        mask.push(false);
                        cur.push_byte(c)
                            .change_context(ParseError::InvalidChar { ch: 0 })?;
                        mask.push(false);
                    }
                    None => return Err(report_unexpected_eof(line, start)),
                }
            }
            Some(c) => {
                *pos += 1;
                cur.push_byte(c)
                    .change_context(ParseError::InvalidChar { ch: 0 })?;
                mask.push(false);
            }
            None => return Err(report_unexpected_eof(line, start)),
        }
    }
}
