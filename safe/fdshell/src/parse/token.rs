mod step;

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
        if st.in_quotes {
            if !super::quotes::handle_quoted_char(b, &mut st.cur, &mut bytes, line, &mut st.pos)? {
                st.in_quotes = false;
                st.quote_start = None;
            }
        } else {
            step::unquoted(b, &mut st, line, &mut bytes)?;
        }
    }
    st.finish(line)
}

/// Accumulator for the byte-by-byte tokenization loop.
pub(super) struct State {
    pub(super) tokens: Vec<Token>,
    pub(super) cur: ShortCStr,
    pub(super) in_quotes: bool,
    pub(super) quote_start: Option<usize>,
    pub(super) fq: bool,
    pub(super) word_started: bool,
    pub(super) start: usize,
    pub(super) pos: usize,
}

impl State {
    pub(super) fn new() -> Self {
        Self {
            tokens: Vec::new(),
            cur: ShortCStr::new(),
            in_quotes: false,
            quote_start: None,
            fq: false,
            word_started: false,
            start: 0,
            pos: 0,
        }
    }

    /// End of line: an open quote is an error; otherwise flush the tail.
    fn finish(mut self, line: &[u8]) -> Result<Vec<Token>, Report<ParseError>> {
        if self.in_quotes {
            return Err(report_unbalanced_quote(line, self.quote_start.unwrap_or(0)));
        }
        if !self.cur.is_empty() {
            self.tokens
                .push((self.cur, self.start, line.len(), self.fq));
        }
        Ok(self.tokens)
    }
}
