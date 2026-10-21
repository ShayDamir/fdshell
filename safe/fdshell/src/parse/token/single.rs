//! The single-quote span state machine (task #141).
//!
//! A `'…'` span contributes literal bytes to the word and quoted mask bits —
//! exactly the double-quote meaning: the bytes are protected from IFS splitting
//! and globbing, and the span is one quote region (its first byte is a
//! `QuotedStart`, task #285). The bit carries no quote *kind* beyond that, so
//! the literal is written in the double-quote escape form (`\` → `\\`, `$` →
//! `\$`, `` ` `` → `` \` ``, any other byte verbatim): both readers of word text
//! (`substitute/arg.rs`'s quoted arm, `bytes/fold.rs::fold_mask`) fold those
//! pairs back to the literal
//! byte, and the unquoted arms fold them the same way, so the encoding is
//! correct with and without the mask (LESSONS: a single-quoted literal is
//! stored in the double-quote escape form). A backtick needs the pair for the
//! same reason as `$`: without it the substitution pass would read the byte as
//! a command-substitution delimiter (task #216).

use super::State;
use crate::bytes::SQUOTE;
use crate::error::parse::ParseError;
use error_stack::Report;

impl State {
    /// Open a `'…'` span on the current byte: the span is quoted word content,
    /// so a leading quote makes the word fully quoted (`fq`) and marks it as a
    /// word that counts even when it accumulates no bytes (`""` and `''`).
    pub(super) fn open_single(&mut self) {
        if self.cur.is_empty() {
            self.fq = true;
        }
        self.word_quoted = true;
        self.in_single = true;
        self.word_started = true;
        self.span_open = self.cur.len();
        self.single_start = Some(self.pos - 1);
    }

    /// Handle one byte of an open `'…'` span: returns `false` for the closing
    /// quote. Inside the span a backslash is literal (POSIX §2.2.3 — no escape
    /// pair, no line continuation), and `$` and backticks start no expansion;
    /// `\`, `$` and `` ` `` go in as the escape pair their reader folds back.
    pub(super) fn single_char(&mut self, b: u8) -> Result<bool, Report<ParseError>> {
        if b == SQUOTE {
            return Ok(false);
        }
        if matches!(b, b'\\' | b'$' | b'`') {
            self.push_masked(b'\\', true)?;
        }
        self.push_masked(b, true)?;
        Ok(true)
    }

    /// The `$'…'` form: the `$` is written escaped (`\$`, mask `false`) and the
    /// quote is left to the next iteration, where it opens its span. The
    /// result is the POSIX-sh reading — `$'x'` is the literal `$x`, not bash's
    /// ANSI-C quoting (accepted divergence, see the README; task #156).
    pub(super) fn dollar_quote(&mut self) -> Result<(), Report<ParseError>> {
        self.word_start();
        self.push_masked(b'\\', false)?;
        self.push_masked(b'$', false)
    }
}
