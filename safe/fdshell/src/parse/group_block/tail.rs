//! The group's **tail**: the redirections written after the closing brace.
//! Extracted from `parse/group_block.rs` for the line budget (task #354); the
//! closer logic (`split_close`) stays with the closer rule.

use super::Token;
use crate::error::parse::ParseError;
use crate::parse::classify::parse_redirect;
use crate::redirect::RedirectDef;
use crate::scan::heredoc::span_operator;
use alloc::vec::Vec;
use error_stack::{Report, bail};

/// The redirections written after `}`, in source order. A here-doc/here-string
/// operator is checked first (it starts with `<`, which the read redirect would
/// otherwise swallow); a pipeline, background, here-doc, or here-string form on
/// the group is a parse error (each is a separate task), and so is a trailing
/// plain word. The here-doc test is the shared raw-span shape rule, so the
/// io-number form `{ …; } 3<<T` keeps the here-doc message instead of degrading
/// to a trailing word.
pub(super) fn redirects(
    tail: &[Token],
    line: &[u8],
) -> Result<Vec<RedirectDef>, Report<ParseError>> {
    let mut redirects = Vec::new();
    let mut i = 0;
    while i < tail.len() {
        let (t, start, end, _fq, _mask, _spans) = tail.get(i).ok_or(ParseError::Never)?;
        if t.starts_with(b"<<") || span_operator(line, *start, *end).is_some() {
            bail!(ParseError::GroupHereDocNotSupported);
        }
        if let Some((def, extra)) = parse_redirect(tail, i)? {
            redirects.push(def);
            i += extra + 1;
            continue;
        }
        if t.eq_bytes(b"|") {
            bail!(ParseError::UnexpectedPipe);
        }
        if t.starts_with(b"&") {
            bail!(ParseError::UnexpectedChar { ch: b'&' });
        }
        bail!(ParseError::GroupTrailingWord);
    }
    Ok(redirects)
}

#[cfg(test)]
mod tests;
