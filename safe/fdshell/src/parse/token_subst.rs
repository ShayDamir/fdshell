use super::token::State;
use crate::error::parse::{ParseError, report_unexpected_eof};
use crate::paren_scan::scan_dollar_paren_body;
use error_stack::{Report, ResultExt};

impl State {
    /// Read a `$(…)` command substitution into the current word.
    pub(super) fn read_dollar_paren(
        &mut self,
        bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
        line: &[u8],
    ) -> Result<(), Report<ParseError>> {
        let start = self.pos - 1;
        self.cur
            .push_byte(b'$')
            .change_context(ParseError::InvalidChar { ch: 0 })?;
        self.mask.push(false);
        self.cur
            .push_byte(b'(')
            .change_context(ParseError::InvalidChar { ch: 0 })?;
        self.mask.push(false);
        bytes.next(); // consume '('
        self.pos += 1;
        let Some(body) = scan_dollar_paren_body(bytes) else {
            return Err(report_unexpected_eof(line, start));
        };
        // The stream consumed the body plus the closing `)`.
        self.pos += body.len() + 1;
        for _ in &body {
            self.mask.push(false);
        }
        self.cur
            .push_checked(&body)
            .change_context(ParseError::InvalidChar { ch: 0 })?;
        self.cur
            .push_byte(b')')
            .change_context(ParseError::InvalidChar { ch: 0 })?;
        self.mask.push(false);
        Ok(())
    }
}
