//! The bare `&>` / `&>>` operator token: the token-level twin of
//! `parse::redirect::bare`, sharing its operand rule.

use super::BgRedirectResult;
use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::redirect::{RedirectDef, RedirectDirection};
use crate::scan::heredoc::invalid_delimiter;
use alloc::vec;
use error_stack::{Report, bail};

/// The bare combined redirect at `i`: its operand is the next token and one
/// open of it is shared by fd 1 and fd 2 (bash's `&> file` is `> file 2>&1`),
/// with the operator's direction. The operand is a plain word carrying its own
/// quote mask; a missing operand, a separator or another operator is an
/// `invalid redirect` syntax error.
pub(super) fn parse(
    tokens: &[Token],
    i: usize,
    direction: RedirectDirection,
) -> Result<BgRedirectResult, Report<ParseError>> {
    let Some(operand) = tokens.get(i + 1) else {
        bail!(ParseError::InvalidRedirect);
    };
    let (word, _s, _e, _fq, omask, _spans) = operand;
    if invalid_delimiter(word, omask) {
        bail!(ParseError::InvalidRedirect);
    }
    // A `/dev/fd/N` operand stays a path source: the attached `&>/dev/fd/N`
    // form has always opened the magic path, and the kernel dup is the same
    // effect (the `>` family's `RedirectDef::dup` is recorded in LESSONS).
    let source = crate::parse::redirect::path_word::full(operand);
    Ok(BgRedirectResult {
        redirects: vec![RedirectDef::combined(direction, source)],
        pidvar: None,
        bg_force: false,
    })
}
