use crate::error::parse::{ParseError, report_unexpected_eof};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(crate) fn handle_quoted_char(
    b: u8,
    cur: &mut ShortCStr,
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    line: &[u8],
    pos: usize,
) -> Result<bool, Report<ParseError>> {
    match b {
        b'"' => Ok(false),
        b'\\' => {
            if let Some(c) = bytes.next() {
                cur.push(c)
                    .change_context(ParseError::InvalidChar { ch: 0 })?;
            } else {
                return Err(report_unexpected_eof(line, pos));
            }
            Ok(true)
        }
        _ => {
            cur.push(b)
                .change_context(ParseError::InvalidChar { ch: 0 })?;
            Ok(true)
        }
    }
}
