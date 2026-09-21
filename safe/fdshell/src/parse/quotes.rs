use super::token::State;
use crate::error::parse::{ParseError, report_unexpected_eof};
use error_stack::{Report, ResultExt};

impl State {
    /// Handle one byte consumed inside double quotes; returns whether the
    /// quote is still open afterwards.
    pub(super) fn quoted_char(
        &mut self,
        b: u8,
        bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
        line: &[u8],
    ) -> Result<bool, Report<ParseError>> {
        match b {
            b'"' => Ok(false),
            b'\\' => {
                let Some(c) = bytes.next() else {
                    return Err(report_unexpected_eof(line, self.pos));
                };
                self.pos += 1;
                // `\<newline>` is line continuation (both bytes removed); `\` before
                // `"` or a backtick escapes that char; `\\` and `\$` keep both bytes
                // so substitution resolves them (`\\`→`\`, `\$`→a literal,
                // unexpanded `$`); any other `\<char>` keeps the backslash literally.
                match c {
                    b'\n' => Ok(true),
                    b'"' | b'`' => {
                        self.cur
                            .push_byte(c)
                            .change_context(ParseError::InvalidChar { ch: 0 })?;
                        self.mask.push(true);
                        Ok(true)
                    }
                    _ => {
                        self.cur
                            .push_byte(b'\\')
                            .change_context(ParseError::InvalidChar { ch: 0 })?;
                        self.mask.push(true);
                        self.cur
                            .push_byte(c)
                            .change_context(ParseError::InvalidChar { ch: 0 })?;
                        self.mask.push(true);
                        Ok(true)
                    }
                }
            }
            _ => {
                self.cur
                    .push_byte(b)
                    .change_context(ParseError::InvalidChar { ch: 0 })?;
                self.mask.push(true);
                Ok(true)
            }
        }
    }
}
