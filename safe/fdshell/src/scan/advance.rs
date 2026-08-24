use super::ScanState;

impl ScanState {
    /// Fold the character at `i` into the state and return the next position
    /// to scan.
    ///
    /// Toggles quotes and backticks and tracks `$( )` depth. A `$(` pair is
    /// consumed together, advancing two positions.
    pub(crate) fn advance(&mut self, line: &[u8], i: usize) -> usize {
        let b = line.get(i).copied().unwrap_or(0);
        let bare = !self.in_quote && !self.in_backtick;
        if b == b'"' {
            self.in_quote = !self.in_quote;
            self.word_active = true;
            i + 1
        } else if bare && b == b'$' {
            self.word_active = true;
            if line.get(i + 1) == Some(&b'(') {
                self.dollar_paren_depth = self.dollar_paren_depth.saturating_add(1);
                i + 2
            } else {
                i + 1
            }
        } else if bare && b == b'(' {
            let nested = self.dollar_paren_depth > 0;
            if nested {
                self.dollar_paren_depth = self.dollar_paren_depth.saturating_add(1);
            }
            self.word_active = nested;
            i + 1
        } else if bare && b == b')' {
            let closed_sub = self.dollar_paren_depth > 0;
            self.dollar_paren_depth = self.dollar_paren_depth.saturating_sub(1);
            self.word_active = closed_sub;
            i + 1
        } else if bare && b == b'`' {
            self.in_backtick = true;
            self.word_active = true;
            i + 1
        } else if self.in_backtick && b == b'`' {
            self.in_backtick = false;
            self.word_active = true;
            i + 1
        } else {
            self.word_active = !is_word_break(b);
            i + 1
        }
    }
}

/// A byte that ends the current word, so the next byte starts a new one
/// (whitespace or an unquoted shell metacharacter).
pub(super) fn is_word_break(b: u8) -> bool {
    b.is_ascii_whitespace() || matches!(b, b';' | b'|' | b'&' | b'<' | b'>')
}
