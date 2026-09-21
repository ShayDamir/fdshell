use alloc::vec::Vec;

use super::token::State;
use crate::error::parse::ParseError;
use error_stack::{Report, ResultExt};

impl State {
    /// Handle pipe character `|`. Returns whether a pipe token was emitted.
    pub(super) fn pipe_token(&mut self) -> Result<bool, Report<ParseError>> {
        let is_redir =
            (self.cur.starts_with(b"%") || self.cur.starts_with(b"&")) && self.cur.ends_with(b">");
        if is_redir {
            self.cur
                .push_byte(b'|')
                .change_context(ParseError::InvalidChar { ch: 0 })?;
            self.mask.push(false);
            Ok(false)
        } else {
            self.emit(self.pos - 1);
            self.tokens
                .push((c"|".into(), self.pos - 1, self.pos, false, Vec::new()));
            Ok(true)
        }
    }
}
