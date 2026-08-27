use alloc::vec::Vec;

use crate::error::parse::{ParseError, report_unexpected_eof};
use crate::paren_scan::scan_dollar_paren_body;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(crate) fn read_dollar_paren(
    line: &[u8],
    cur: &mut ShortCStr,
    mask: &mut Vec<bool>,
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    pos: &mut usize,
) -> Result<(), Report<ParseError>> {
    let start = *pos - 1;
    cur.push_byte(b'$')
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    mask.push(false);
    cur.push_byte(b'(')
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    mask.push(false);
    bytes.next(); // consume '('
    *pos += 1;
    let Some(body) = scan_dollar_paren_body(bytes) else {
        return Err(report_unexpected_eof(line, start));
    };
    // The stream consumed the body plus the closing `)`.
    *pos += body.len() + 1;
    for _ in &body {
        mask.push(false);
    }
    cur.push_checked(&body)
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    cur.push_byte(b')')
        .change_context(ParseError::InvalidChar { ch: 0 })?;
    mask.push(false);
    Ok(())
}
