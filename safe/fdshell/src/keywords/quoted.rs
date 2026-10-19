//! The quoted-word rule of keyword recognition (task #141).

use crate::bytes::{QUOTE, SQUOTE};

/// A word written with quote bytes is data, never a keyword: `"fi"`, `'done'`,
/// `"{"` are words (POSIX §2.9.1 recognizes only unquoted keyword tokens, and
/// bash treats a quoted keyword as a command word). The word's raw span is the
/// run ending at `word_end`, back to the byte that breaks a word.
///
/// The one keyword-recognition gate at both granularities: [`keyword_delta`]
/// opens with it, and the parse layer calls it through
/// `parse::word_quoted::keyword_word` (task #247), so the block-depth scan and
/// the block builders cannot disagree.
pub(crate) fn quoted_word(line: &[u8], word_end: usize) -> bool {
    let mut i = word_end;
    let mut quoted = false;
    while i > 0 {
        match line.get(i - 1).copied() {
            Some(b' ' | b'\t' | b'\n' | b';' | b'|' | b'&' | b'<' | b'>' | b'(' | b')') => break,
            Some(b) => quoted |= b == QUOTE || b == SQUOTE,
            None => break,
        }
        i -= 1;
    }
    quoted
}
