// mutants: -C --lib -C --test -C loops -C --test -C backslash -C --test -C cond_blocks -- -E 'test(/^parse::/) | binary(loops) | binary(backslash) | binary(cond_blocks)'

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

    /// A `\`+newline pair at the **start** of a word is bash's line-continuation
    /// join, which bash performs before tokenization: both bytes are dropped
    /// here and the word starts after the pair, so `for i in 1 2 ;\` + newline +
    /// `do …` is the plain one-`;` spelling every block builder already accepts.
    /// Anywhere else the pair stays in the word text — a word's `start`/`end`
    /// index the raw line, and the folding surfaces
    /// (`bytes/fold/raw.rs::fold_impl`, `bytes/fold.rs::fold_mask`,
    /// `substitute/arg/escape.rs::unquoted`) are what drop it there.
    /// Returns whether the pair was consumed.
    pub(super) fn read_line_continuation(
        &mut self,
        bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    ) -> bool {
        let at_word_start = self.cur.is_empty() && !self.word_quoted;
        if !at_word_start || bytes.peek() != Some(&b'\n') {
            return false;
        }
        bytes.next();
        self.pos += 1;
        self.start = self.pos;
        true
    }
}
