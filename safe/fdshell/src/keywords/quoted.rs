//! The quoted-word rule of keyword recognition (task #141).
//!
//! Two functions, one rule seen from two sides: [`quoted_word`] measures a word's
//! own raw span, and [`keyword_is_quoted`] is keyword recognition's gate — it
//! measures the keyword's **own** bytes, so the bytes behind the byte that breaks
//! the word belong to what is glued to it (bash splits `done>f` at the `>`).

use crate::bytes::{QUOTE, SQUOTE, word_break};

/// A word written with quote bytes is data, never a keyword: `"fi"`, `'done'`,
/// `"{"` are words (POSIX §2.9.1 recognizes only unquoted keyword tokens, and
/// bash treats a quoted keyword as a command word). The word's raw span is the
/// run ending at `word_end`, back to the byte that breaks a word.
///
/// The one **span** gate at both granularities: the parse layer calls it through
/// `parse::word_quoted::keyword_word` (task #247) and the definer through
/// `keywords::function`, so the block-depth scan and the block builders cannot
/// disagree. Keyword recognition calls [`keyword_is_quoted`], which defers here for
/// a word that is nothing but its keyword.
pub(crate) fn quoted_word(line: &[u8], word_end: usize) -> bool {
    let mut i = word_end;
    let mut quoted = false;
    while i > 0 {
        match line.get(i - 1).copied() {
            Some(b) if word_break(b) => break,
            Some(b) => quoted |= b == QUOTE || b == SQUOTE,
            None => break,
        }
        i -= 1;
    }
    quoted
}

/// The gate of the recognition table [`super::kind`] (task #413): a word that
/// carries a byte which **breaks** it is a keyword glued to an operand —
/// `done>f"f"`, `fi>f`, `for(("$x")`, `a&&if>x"y"` — and the row that matched the
/// keyword's literal prefix proves those bytes carry no quote byte, so the word
/// **is** the keyword: what sits behind the breaking byte is the operand's, and
/// bash splits the word there. Only a word that is nothing but its keyword word is
/// measured on the span, by [`quoted_word`].
pub(crate) fn keyword_is_quoted(word: &[u8], line: &[u8], word_end: usize) -> bool {
    if word.iter().any(|&b| word_break(b)) {
        return false;
    }
    quoted_word(line, word_end)
}
