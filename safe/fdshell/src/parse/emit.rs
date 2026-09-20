use super::Token;
use alloc::vec::Vec;
use sys::ShortCStr;

/// Emit the current token buffer as a token, then reset it.
///
/// The word may be empty: a word started by double quotes (`""`) is one
/// empty word, reported via `word_quoted`.
///
/// `end` is the exclusive byte position after the token's raw (quoted) text.
pub fn emit_token(
    tokens: &mut Vec<Token>,
    cur: &mut ShortCStr,
    token_start: usize,
    end: usize,
    fully_quoted: bool,
    word_quoted: bool,
    mask: &mut Vec<bool>,
) {
    if !cur.is_empty() || word_quoted {
        tokens.push((
            core::mem::take(cur),
            token_start,
            end,
            fully_quoted,
            core::mem::take(mask),
        ));
    }
}
