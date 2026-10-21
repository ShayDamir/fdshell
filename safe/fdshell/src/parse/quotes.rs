use super::token::State;
use crate::error::parse::{ParseError, report_unexpected_eof};
use error_stack::Report;

mod subst;

impl State {
    /// Handle one byte consumed inside double quotes; returns whether the
    /// quote is still open afterwards.
    ///
    /// A nested `$( … )`, `${ … }` or `` ` … ` `` region is scanned as one
    /// unit (`subst`): its own `"` are markers of the *inner* region, so they
    /// never close the outer quoted region, and its bytes are pushed with
    /// mask bit `true`.
    pub(super) fn quoted_char(
        &mut self,
        b: u8,
        bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
        line: &[u8],
    ) -> Result<bool, Report<ParseError>> {
        match b {
            b'"' => Ok(false),
            b'$' if bytes.peek() == Some(&b'(') => {
                subst::dollar_paren(self, bytes, line)?;
                Ok(true)
            }
            b'$' if bytes.peek() == Some(&b'{') => {
                subst::brace(self, bytes, line)?;
                Ok(true)
            }
            b'`' => {
                subst::backtick(self, bytes, line)?;
                Ok(true)
            }
            b'\\' => {
                let Some(c) = bytes.next() else {
                    return Err(report_unexpected_eof(line, self.pos));
                };
                self.pos += 1;
                // `\<newline>` is line continuation (both bytes removed); `\`
                // before `"` escapes that char; `\\`, `\$` and `` \` `` keep both
                // bytes so substitution resolves them (`\\`→`\`, `\$`→a literal,
                // unexpanded `$`, `` \` ``→a literal backtick that is never a
                // command-substitution delimiter — task #216); any other `\<char>`
                // keeps the backslash literally.
                match c {
                    b'\n' => Ok(true),
                    b'"' => {
                        self.push_masked(c, true)?;
                        Ok(true)
                    }
                    _ => {
                        self.push_masked(b'\\', true)?;
                        self.push_masked(c, true)?;
                        Ok(true)
                    }
                }
            }
            _ => {
                self.push_masked(b, true)?;
                Ok(true)
            }
        }
    }
}

#[cfg(test)]
mod tests;
