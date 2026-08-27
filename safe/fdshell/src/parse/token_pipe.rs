use alloc::vec::Vec;

use super::Token;
use crate::error::parse::ParseError;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use super::emit::emit_token;

/// Handle pipe character `|`. Returns whether a pipe token was emitted.
pub(crate) fn handle_pipe(
    tokens: &mut Vec<Token>,
    cur: &mut ShortCStr,
    token_start: usize,
    token_fully_quoted: bool,
    pos: usize,
    mask: &mut Vec<bool>,
) -> Result<bool, Report<ParseError>> {
    let is_redir = (cur.starts_with(b"%") || cur.starts_with(b"&")) && cur.ends_with(b">");
    if is_redir {
        cur.push_byte(b'|')
            .change_context(ParseError::InvalidChar { ch: 0 })?;
        mask.push(false);
        Ok(false)
    } else {
        emit_token(tokens, cur, token_start, pos - 1, token_fully_quoted, mask);
        tokens.push((c"|".into(), pos - 1, pos, false, Vec::new()));
        Ok(true)
    }
}
