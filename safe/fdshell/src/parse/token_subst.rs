use super::token::State;
use crate::error::parse::{ParseError, report_unexpected_eof};
use crate::paren_scan::scan_dollar_paren_body;
use error_stack::Report;

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
        self.push_masked(b'$', quoted)?;
        self.push_masked(b'(', quoted)?;
        bytes.next(); // consume '('
        self.pos += 1;
        let Some(body) = scan_dollar_paren_body(bytes) else {
            return Err(report_unexpected_eof(line, start));
        };
        // The stream consumed the body plus the closing `)`.
        self.pos += body.len() + 1;
        self.push_masked_run(&body, quoted)?;
        self.push_masked(b')', quoted)?;
        Ok(())
    }
}
