use alloc::vec::Vec;

use super::super::comment::skip_comment;
use super::State;
use crate::bytes::{QUOTE, SQUOTE};
use crate::error::parse::ParseError;
use error_stack::Report;

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
                    self.tokens.push((
                        sep.into(),
                        self.pos - 1,
                        self.pos,
                        false,
                        Vec::new(),
                        Vec::new(),
                    ));
                }
            }
            b'|' => {
                if self.pipe_token()? {
                    self.word_reset();
                    self.start = self.pos;
                }
            }
            QUOTE => {
                if self.cur.is_empty() {
                    self.fq = true;
                }
                self.word_quoted = true;
                self.in_quotes = true;
                self.word_started = true;
                self.span_open = self.cur.len();
                self.quote_start = Some(self.pos - 1);
            }
            // A `'…'` span is literal word content: both quote bytes are
            // stripped and its bytes are masked, like a double-quoted span.
            SQUOTE => self.open_single(),
            // `$'…'` is the POSIX-sh reading (`$'x'` → literal `$x`), not
            // bash's ANSI-C quoting: the `$` is escaped and the quote opens.
            b'$' if bytes.peek() == Some(&SQUOTE) => self.dollar_quote()?,
            b'$' if bytes.peek() == Some(&b'(') => {
                self.word_start();
                self.read_dollar_paren(bytes, line, false)?;
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
            // An unquoted escape pair: the pair is a word byte pair, so an
            // escaped `#` never starts a comment and a leading `\if` is never
            // a keyword token.
            b'\\' => {
                self.word_start();
                self.read_escape(bytes)?;
            }
            _ => {
                self.word_start();
                self.push_masked(b, false)?;
            }
        }
        Ok(())
    }

    /// Word state after a separator: the next byte starts a fresh word, and the
    /// first quoted byte of the new word opens a quote region.
    fn word_reset(&mut self) {
        self.region_open = false;
        self.fq = false;
        self.word_quoted = false;
        self.word_started = false;
        self.spans = Vec::new();
    }

    /// The current word starts unquoted.
    pub(super) fn word_start(&mut self) {
        self.fq = false;
        self.word_started = true;
    }
}
