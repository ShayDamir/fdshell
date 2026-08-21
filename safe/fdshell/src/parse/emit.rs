use super::Token;
use alloc::vec::Vec;
use sys::ShortCStr;

/// Emit the current token buffer as a token, then reset it.
///
/// `end` is the exclusive byte position after the token's raw (quoted) text.
pub fn emit_token(
    tokens: &mut Vec<Token>,
    cur: &mut ShortCStr,
    token_start: usize,
    end: usize,
    fully_quoted: bool,
) {
    if !cur.is_empty() {
        tokens.push((core::mem::take(cur), token_start, end, fully_quoted));
    }
}
