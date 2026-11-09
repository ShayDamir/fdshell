// mutants: -C --lib -C --test -C block_redirect -C --test -C loops -- -E 'test(/^parse::/) | binary(block_redirect) | binary(loops)'

use super::Token;
use super::token::State;

impl State {
    /// Emit the current token buffer as a token, then reset it.
    ///
    /// The word may be empty: a word started by double quotes (`""`) is one
    /// empty word, reported via `word_quoted`.
    ///
    /// `end` is the exclusive byte position after the token's raw (quoted) text.
    pub(super) fn emit(&mut self, end: usize) {
        if !self.cur.is_empty() || self.word_quoted {
            self.tokens.push(Token {
                word: core::mem::take(&mut self.cur),
                start: self.start,
                end,
                fully_quoted: self.fq,
                mask: core::mem::take(&mut self.mask),
                spans: core::mem::take(&mut self.spans),
            });
        }
    }
}
