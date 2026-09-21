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
            self.tokens.push((
                core::mem::take(&mut self.cur),
                self.start,
                end,
                self.fq,
                core::mem::take(&mut self.mask),
            ));
        }
    }
}
