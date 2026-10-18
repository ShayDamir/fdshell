mod escape;
mod single;
mod statement;
mod step;

pub(crate) use statement::tokenize_statement;

use super::Token;
use crate::error::parse::{ParseError, report_unbalanced_quote};
use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;

pub fn tokenize(line: &[u8]) -> Result<Vec<Token>, Report<ParseError>> {
    let mut st = State::new();
    let mut bytes = line.iter().copied().peekable();
    while let Some(b) = bytes.next() {
        st.pos += 1;
        if st.in_single {
            if !st.single_char(b)? {
                st.in_single = false;
                st.single_start = None;
                st.close_span();
            }
        } else if st.in_quotes {
            if !st.quoted_char(b, &mut bytes, line)? {
                st.in_quotes = false;
                st.quote_start = None;
                st.close_span();
            }
        } else {
            st.unquoted(b, line, &mut bytes)?;
        }
    }
    st.finish(line)
}

/// Accumulator for the byte-by-byte tokenization loop.
pub(super) struct State {
    pub(super) tokens: Vec<Token>,
    pub(super) cur: ShortCStr,
    /// Per-byte quote mask, parallel to `cur`: `true` for bytes consumed
    /// inside double or single quotes (protected from IFS word splitting).
    pub(super) mask: Vec<bool>,
    /// Word-text indices at which a quote span closed with **no bytes**
    /// (`""`, `''`): invisible to the mask, recorded for the tilde rule
    /// (task #321, `substitute/tilde.rs`).
    pub(super) spans: Vec<usize>,
    /// Word-text length when the current open quote pair started; a close at
    /// this length contributed no byte, so the index is recorded.
    pub(super) span_open: usize,
    pub(super) in_quotes: bool,
    pub(super) quote_start: Option<usize>,
    /// Inside a `'…'` span: its bytes are literal and masked (see
    /// [`single`]).
    pub(super) in_single: bool,
    /// Byte index of the open `'`, for the unbalanced-quote report.
    pub(super) single_start: Option<usize>,
    pub(super) fq: bool,
    /// The current word contained double quotes, so it counts as one word
    /// even when it accumulated no bytes (e.g. `""`).
    pub(super) word_quoted: bool,
    pub(super) word_started: bool,
    pub(super) start: usize,
    pub(super) pos: usize,
}

impl State {
    pub(super) fn new() -> Self {
        Self {
            tokens: Vec::new(),
            cur: ShortCStr::new(),
            mask: Vec::new(),
            spans: Vec::new(),
            span_open: 0,
            in_quotes: false,
            quote_start: None,
            in_single: false,
            single_start: None,
            fq: false,
            word_quoted: false,
            word_started: false,
            start: 0,
            pos: 0,
        }
    }

    /// A quote pair just closed: if it contributed no byte (`cur` is exactly
    /// as long as when the pair opened), its opening index goes into `spans`
    /// — the mask has no bit for a span of zero bytes (task #321).
    pub(super) fn close_span(&mut self) {
        if self.cur.len() == self.span_open {
            self.spans.push(self.span_open);
        }
    }

    /// End of line: an open quote is an error; otherwise flush the tail.
    fn finish(mut self, line: &[u8]) -> Result<Vec<Token>, Report<ParseError>> {
        if self.in_quotes {
            return Err(report_unbalanced_quote(line, self.quote_start.unwrap_or(0)));
        }
        if self.in_single {
            return Err(report_unbalanced_quote(
                line,
                self.single_start.unwrap_or(0),
            ));
        }
        self.emit(line.len());
        Ok(self.tokens)
    }
}

#[cfg(test)]
mod tests;
