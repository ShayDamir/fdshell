//! The tokenizer's [`State`] accumulator: the word under construction, its
//! per-byte quote mask, the quote spans that contributed no bytes, and the
//! quote-state flags. The struct is the shared state of the byte-by-byte loop,
//! so every piece of the loop lives in an `impl State` block of its own module:
//! [`super::step`], [`super::escape`], [`super::single`], [`super::mask`] under
//! `parse/token/`, and [`crate::parse::quotes`], [`crate::parse::emit`],
//! [`crate::parse::token_subst`], [`crate::parse::token_pipe`],
//! [`crate::parse::backtick`] beside `parse/token.rs` — which is why the state
//! and its fields are `pub(in crate::parse)`: exactly the reach they had while
//! the struct itself sat in `parse/token.rs`. This file holds the state's
//! shape, its seed, and the end-of-line flush.

use super::Token;
use crate::error::parse::{ParseError, report_unbalanced_quote};
use crate::mask::MaskBit;
use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;

/// Accumulator for the byte-by-byte tokenization loop.
pub(in crate::parse) struct State {
    pub(in crate::parse) tokens: Vec<Token>,
    pub(in crate::parse) cur: ShortCStr,
    /// Per-byte quote mask, parallel to `cur`: the quote state of every word
    /// byte (protected from IFS word splitting), with the byte that **starts**
    /// a quote region spelled out ([`super::mask`], task #285).
    pub(in crate::parse) mask: Vec<MaskBit>,
    /// Word-text indices at which a quote span closed with **no bytes**
    /// (`""`, `''`): invisible to the mask, recorded for the tilde rule
    /// (task #321, `substitute/tilde.rs`).
    pub(in crate::parse) spans: Vec<usize>,
    /// Word-text length when the current open quote pair started; a close at
    /// this length contributed no byte, so the index is recorded.
    pub(in crate::parse) span_open: usize,
    /// A quote region is open: the next quoted byte continues it, so it is not
    /// a region start ([`super::mask`], task #285).
    pub(in crate::parse) region_open: bool,
    pub(in crate::parse) in_quotes: bool,
    pub(in crate::parse) quote_start: Option<usize>,
    /// Inside a `'…'` span: its bytes are literal and masked (see
    /// [`super::single`]).
    pub(in crate::parse) in_single: bool,
    /// Byte index of the open `'`, for the unbalanced-quote report.
    pub(in crate::parse) single_start: Option<usize>,
    pub(in crate::parse) fq: bool,
    /// The current word contained double quotes, so it counts as one word
    /// even when it accumulated no bytes (e.g. `""`).
    pub(in crate::parse) word_quoted: bool,
    pub(in crate::parse) word_started: bool,
    pub(in crate::parse) start: usize,
    pub(in crate::parse) pos: usize,
}

impl State {
    pub(in crate::parse) fn new() -> Self {
        Self {
            tokens: Vec::new(),
            cur: ShortCStr::new(),
            mask: Vec::new(),
            spans: Vec::new(),
            span_open: 0,
            region_open: false,
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

    /// End of line: an open quote is an error; otherwise flush the tail.
    /// Reachable only inside `parse::token` — the loop's own `tokenize` is its
    /// single caller, as it was while this `impl` sat in `parse/token.rs`.
    pub(in crate::parse::token) fn finish(
        mut self,
        line: &[u8],
    ) -> Result<Vec<Token>, Report<ParseError>> {
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
