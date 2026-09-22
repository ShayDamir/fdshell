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
            if !st.quoted_char(b, &mut bytes, line)? {
                st.in_quotes = false;
                st.quote_start = None;
            }
        } else {
            st.unquoted(b, line, &mut bytes)?;
        }
    }
    st.finish(line)
}

/// Tokenize a statement whose heredoc bodies are opaque: the body spans are
/// blanked in a copy first, so arbitrary body bytes — including unbalanced
/// quotes — never break tokenization. Byte offsets are preserved.
pub(crate) fn tokenize_statement(line: &[u8]) -> Result<Vec<Token>, Report<ParseError>> {
    let regions = crate::scan::heredoc::body_regions(line);
    if regions.is_empty() {
        return tokenize(line);
    }
    let mut masked: Vec<u8> = line.to_vec();
    for (start, end) in regions {
        if let Some(body) = masked.get_mut(start..end) {
            for b in body.iter_mut() {
                *b = b' ';
            }
        }
    }
    tokenize(&masked)
}

/// Accumulator for the byte-by-byte tokenization loop.
pub(super) struct State {
    pub(super) tokens: Vec<Token>,
    pub(super) cur: ShortCStr,
    /// Per-byte quote mask, parallel to `cur`: `true` for bytes consumed
    /// inside double quotes (protected from IFS word splitting).
    pub(super) mask: Vec<bool>,
    pub(super) in_quotes: bool,
    pub(super) quote_start: Option<usize>,
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
            in_quotes: false,
            quote_start: None,
            fq: false,
            word_quoted: false,
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
        self.emit(line.len());
        Ok(self.tokens)
    }
}
