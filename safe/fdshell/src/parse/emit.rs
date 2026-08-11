use alloc::vec::Vec;
use sys::ShortCStr;

/// Emit the current token buffer as a token, then reset it.
pub fn emit_token(
    tokens: &mut Vec<(ShortCStr, usize, bool)>,
    cur: &mut ShortCStr,
    token_start: usize,
    fully_quoted: bool,
) {
    if !cur.is_empty() {
        tokens.push((core::mem::take(cur), token_start, fully_quoted));
    }
}
