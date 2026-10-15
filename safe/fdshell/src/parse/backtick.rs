use super::token::State;
use crate::error::parse::{ParseError, report_unexpected_eof};
use error_stack::{Report, ResultExt};

impl State {
    /// Read a `` `…` `` backtick substitution into the current word.
    pub(super) fn read_backtick(
        &mut self,
        bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
        line: &[u8],
    ) -> Result<(), Report<ParseError>> {
        let start = self.pos - 1;
        self.cur
            .push_byte(b'`')
            .change_context(ParseError::InvalidChar { ch: 0 })?;
        self.mask.push(false);
        loop {
            match bytes.next() {
                Some(b'`') => {
                    self.pos += 1;
                    self.cur
                        .push_byte(b'`')
                        .change_context(ParseError::InvalidChar { ch: 0 })?;
                    self.mask.push(false);
                    return Ok(());
                }
                Some(b'\\') => {
                    self.pos += 1;
                    // The escape pair is kept verbatim, like `scan_backtick`'s
                    // body: the backslash shields the byte in this scan and
                    // reaches the child, which re-parses the body quote-aware
                    // (so `` \` `` is a literal backtick, never a closer).
                    match bytes.next() {
                        Some(c) => {
                            self.pos += 1;
                            self.cur
                                .push_byte(b'\\')
                                .change_context(ParseError::InvalidChar { ch: 0 })?;
                            self.mask.push(false);
                            self.cur
                                .push_byte(c)
                                .change_context(ParseError::InvalidChar { ch: 0 })?;
                            self.mask.push(false);
                        }
                        None => return Err(report_unexpected_eof(line, start)),
                    }
                }
                Some(c) => {
                    self.pos += 1;
                    self.cur
                        .push_byte(c)
                        .change_context(ParseError::InvalidChar { ch: 0 })?;
                    self.mask.push(false);
                }
                None => return Err(report_unexpected_eof(line, start)),
            }
        }
    }
}
