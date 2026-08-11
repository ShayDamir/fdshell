use crate::error::parse::ParseError;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use super::emit::emit_token;

/// Handle pipe character `|`. Returns whether a pipe token was emitted.
pub(crate) fn handle_pipe(
    tokens: &mut alloc::vec::Vec<(ShortCStr, usize, bool)>,
    cur: &mut ShortCStr,
    token_start: usize,
    token_fully_quoted: bool,
    pos: usize,
) -> Result<bool, Report<ParseError>> {
    let is_redir = (cur.starts_with(b"%") || cur.starts_with(b"&")) && cur.ends_with(b">");
    if is_redir {
        cur.push_byte(b'|')
            .change_context(ParseError::InvalidChar { ch: 0 })?;
        Ok(false)
    } else {
        emit_token(tokens, cur, token_start, token_fully_quoted);
        tokens.push((c"|".into(), pos - 1, false));
        Ok(true)
    }
}
