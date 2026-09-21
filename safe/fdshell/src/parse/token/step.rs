use alloc::vec::Vec;

use super::super::comment::skip_comment;
use super::State;
use crate::error::parse::ParseError;
use error_stack::{Report, ResultExt};

impl State {
    /// Handle one unquoted byte of the line, updating the token state.
    pub(super) fn unquoted(
        &mut self,
        b: u8,
        line: &[u8],
        bytes: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    ) -> Result<(), Report<ParseError>> {
        match b {
            b' ' | b'\t' | b';' | b'\n' | b')' => {
                let needs_sep = b == b';' || b == b'\n' || b == b')';
                let sep = if b == b')' { c")" } else { c";" };
                self.emit(self.pos - 1);
                self.word_reset();
                self.start = self.pos;
                if needs_sep {
                    self.tokens
                        .push((sep.into(), self.pos - 1, self.pos, false, Vec::new()));
                }
            }
            b'|' => {
                if self.pipe_token()? {
                    self.word_reset();
                }
            }
            b'"' => {
                if self.cur.is_empty() {
                    self.fq = true;
                }
                self.word_quoted = true;
                self.in_quotes = true;
                self.word_started = true;
                self.quote_start = Some(self.pos - 1);
            }
            b'$' if bytes.peek() == Some(&b'(') => {
                self.word_start();
                self.read_dollar_paren(bytes, line)?;
            }
            b'`' => {
                self.word_start();
                self.read_backtick(bytes, line)?;
            }
            // `#` starts a comment only at the beginning of a word; inside a
            // word it is a literal byte (colors, URLs, `${#var}`, …).
            b'#' if !self.word_started => {
                // No token has accumulated yet, so there is nothing to emit.
                let consumed = skip_comment(bytes);
                self.pos += consumed - 1;
                self.fq = false;
                self.word_started = false;
                self.start = self.pos;
            }
            _ => {
                self.word_start();
                self.cur
                    .push_byte(b)
                    .change_context(ParseError::InvalidChar { ch: 0 })?;
                self.mask.push(false);
            }
        }
        Ok(())
    }

    /// Word state after a separator: the next byte starts a fresh word.
    fn word_reset(&mut self) {
        self.fq = false;
        self.word_quoted = false;
        self.word_started = false;
    }

    /// The current word starts unquoted.
    fn word_start(&mut self) {
        self.fq = false;
        self.word_started = true;
    }
}
