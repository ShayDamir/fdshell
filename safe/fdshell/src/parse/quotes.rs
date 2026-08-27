use alloc::vec::Vec;

use crate::error::parse::{ParseError, report_unexpected_eof};
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(crate) fn handle_quoted_char(
    b: u8,
    cur: &mut ShortCStr,
    mask: &mut Vec<bool>,
    bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    line: &[u8],
    pos: &mut usize,
) -> Result<bool, Report<ParseError>> {
    match b {
        b'"' => Ok(false),
        b'\\' => {
            let Some(c) = bytes.next() else {
                return Err(report_unexpected_eof(line, *pos));
            };
            *pos += 1;
            // `\<newline>` is line continuation (both bytes removed); `\` before
            // `"` or a backtick escapes that char; `\\` and `\$` keep both bytes
            // so substitution resolves them (`\\`→`\`, `\$`→a literal,
            // unexpanded `$`); any other `\<char>` keeps the backslash literally.
            match c {
                b'\n' => Ok(true),
                b'"' | b'`' => {
                    cur.push_byte(c)
                        .change_context(ParseError::InvalidChar { ch: 0 })?;
                    mask.push(true);
                    Ok(true)
                }
                _ => {
                    cur.push_byte(b'\\')
                        .change_context(ParseError::InvalidChar { ch: 0 })?;
                    mask.push(true);
                    cur.push_byte(c)
                        .change_context(ParseError::InvalidChar { ch: 0 })?;
                    mask.push(true);
                    Ok(true)
                }
            }
        }
        _ => {
            cur.push_byte(b)
                .change_context(ParseError::InvalidChar { ch: 0 })?;
            mask.push(true);
            Ok(true)
        }
    }
}
