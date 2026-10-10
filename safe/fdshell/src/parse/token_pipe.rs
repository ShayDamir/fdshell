use alloc::vec::Vec;

use super::token::State;
use crate::error::parse::ParseError;
use error_stack::{Report, ResultExt};

impl State {
    /// Handle pipe character `|`. Returns whether a pipe token was emitted.
    pub(super) fn pipe_token(&mut self) -> Result<bool, Report<ParseError>> {
        // A `|` whose current word is `>`-terminated is absorbed into that word:
        // the `%>`/`&>` capture and background operators and the `>|` clobber
        // operator. The rule is `parse::redirect::clobber_word`, the ONE test
        // the byte-level heredoc scan applies to the same raw bytes
        // (`scan/heredoc/ops.rs`), so the two layers cannot disagree (LESSONS:
        // byte-level and token-level rules must agree). The word's quote mask
        // is passed with it, so a quoted `>` never terminates an operator word.
        let is_redir = self
            .cur
            .as_bytes()
            .is_ok_and(|bytes| crate::parse::clobber_word(bytes, &self.mask));
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
