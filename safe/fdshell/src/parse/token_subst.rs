use super::token::State;
use crate::error::parse::{ParseError, report_unexpected_eof};
use crate::paren_scan::scan_dollar_paren_body;
use core::iter::repeat_n;
use error_stack::{Report, ResultExt};
use sys::ShortCStrError;

impl State {
    /// Read a `$(…)` command substitution into the current word.
    ///
    /// `quoted` is the mask bit of the region's bytes: `false` on the unquoted
    /// path, `true` when the region sits inside double quotes (task #130), so
    /// the substitution's output is protected from IFS splitting. The body is
    /// kept verbatim, quote bytes included — `cmd_subst` re-parses it.
    pub(super) fn read_dollar_paren(
        &mut self,
        bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
        line: &[u8],
        quoted: bool,
    ) -> Result<(), Report<ParseError>> {
        let start = self.pos - 1;
        self.push_region_byte(b'$', quoted)
            .change_context(ParseError::InvalidChar { ch: 0 })?;
        self.push_region_byte(b'(', quoted)
            .change_context(ParseError::InvalidChar { ch: 0 })?;
        bytes.next(); // consume '('
        self.pos += 1;
        let Some(body) = scan_dollar_paren_body(bytes) else {
            return Err(report_unexpected_eof(line, start));
        };
        // The stream consumed the body plus the closing `)`.
        self.pos += body.len() + 1;
        self.cur
            .push_checked(&body)
            .change_context(ParseError::InvalidChar { ch: 0 })?;
        self.mask.extend(repeat_n(quoted, body.len()));
        self.push_region_byte(b')', quoted)
            .change_context(ParseError::InvalidChar { ch: 0 })?;
        Ok(())
    }

    /// One delimiter byte of a `$( … )` region: its text byte and its mask bit.
    fn push_region_byte(&mut self, b: u8, quoted: bool) -> Result<(), ShortCStrError> {
        self.cur.push_byte(b)?;
        self.mask.push(quoted);
        Ok(())
    }
}
