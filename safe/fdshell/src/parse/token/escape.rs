use super::State;
use crate::error::parse::ParseError;
use error_stack::Report;

impl State {
    /// Read an unquoted escape pair `\X` into the current word: both bytes go
    /// in, with mask `false` (substitution drops the backslash and marks the
    /// escaped byte protected). Keeping the pair in the word text preserves the
    /// byte offsets and breaks the token-text syntax prefixes (`\if`, `\)`),
    /// so no keyword/redirect/heredoc gate needs a mask-aware rewrite.
    /// A trailing `\` at end of line keeps only the backslash (bash prints a
    /// literal `\` there).
    pub(super) fn read_escape(
        &mut self,
        bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    ) -> Result<(), Report<ParseError>> {
        self.push_masked(b'\\', false)?;
        if let Some(c) = bytes.next() {
            self.pos += 1;
            self.push_masked(c, false)?;
        }
        Ok(())
    }
}
