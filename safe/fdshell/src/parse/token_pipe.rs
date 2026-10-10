use alloc::vec::Vec;

use super::token::State;
use crate::error::parse::ParseError;
use error_stack::{Report, ResultExt};

impl State {
    /// Handle pipe character `|`. Returns whether a pipe token was emitted.
    pub(super) fn pipe_token(&mut self) -> Result<bool, Report<ParseError>> {
        // A `|` whose current word ends at a `>` operator byte is absorbed into
        // that word: `%>`/`&>` (capture / background operator) and the `>|`
        // clobber operator. The same byte rule is applied byte-by-byte by
        // `scan/heredoc/ops.rs` (LESSONS: byte-level and token-level rules
        // must agree), so both key on one `>`-terminated operator test.
        let is_redir = (self.cur.starts_with(b"%") && self.cur.ends_with(b">"))
            || crate::parse::redirect::clobber_prefix(&self.cur);
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
