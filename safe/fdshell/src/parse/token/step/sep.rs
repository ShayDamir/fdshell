//! The separator/terminator arm of the unquoted-byte dispatcher: the five bytes
//! that end a word, three of which also emit a separator token.

use super::State;

impl State {
    /// The `b' ' | b'\t' | b';' | b'\n' | b')'` arm: the word ends at the byte
    /// **before** the separator, the word state resets, and `;`, a newline and
    /// `)` emit their separator token — a newline and a `;` byte become the `;`
    /// token, `)` keeps its own text; a space or tab only ends the word.
    pub(super) fn separator(&mut self, b: u8) {
        let needs_sep = b == b';' || b == b'\n' || b == b')';
        self.emit(self.pos - 1);
        self.word_reset();
        self.start = self.pos;
        if needs_sep {
            let sep = if b == b')' { c")" } else { c";" };
            self.push_separator(sep);
        }
    }
}
