use super::ScanState;

impl ScanState {
    /// Fold the character at `i` into the state and return the next position
    /// to scan.
    ///
    /// Toggles quotes and backticks and tracks `$( )` / `(( ))` depth. A `$(`
    /// pair and a top-level `((` pair are consumed together, advancing two
    /// positions. So is an escape pair `\X`: the backslash shields the next
    /// byte from every byte-level rule, so it never toggles a quote, ends a
    /// word, or closes a paren.
    pub(crate) fn advance(&mut self, line: &[u8], i: usize) -> usize {
        let b = line.get(i).copied().unwrap_or(0);
        let bare = !self.in_quote && !self.in_backtick;
        if b == b'\\' {
            // The pair is consumed together; a trailing `\` at end of line
            // clamps to `line.len()` so the caller's end-of-line boundary
            // still fires and flushes the word.
            self.word_active = true;
            return (i + 2).min(line.len());
        }
        if b == b'"' {
            self.in_quote = !self.in_quote;
            self.word_active = true;
            i + 1
        } else if bare && b == b'$' {
            self.word_active = true;
            if line.get(i + 1) == Some(&b'(') {
                self.paren_depth = self.paren_depth.saturating_add(1);
                i + 2
            } else {
                i + 1
            }
        } else if bare && b == b'(' && self.paren_depth == 0 && line.get(i + 1) == Some(&b'(') {
            // `((…))` arithmetic command: consumed together, like `$(`.
            self.paren_depth = 1;
            self.word_active = true;
            i + 2
        } else if bare && b == b'(' {
            let nested = self.paren_depth > 0;
            if nested {
                self.paren_depth = self.paren_depth.saturating_add(1);
            }
            self.word_active = nested;
            i + 1
        } else if bare && b == b')' {
            let closed_sub = self.paren_depth > 0;
            self.paren_depth = self.paren_depth.saturating_sub(1);
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
