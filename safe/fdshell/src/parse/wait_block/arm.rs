//! One `wait` arm: the `(pattern)` head and its body.

use super::WaitArm;
use super::pattern;
use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::parse::case_clause::extract;
use crate::parse::semi::trim_semi;
use error_stack::Report;
use sys::ScriptText;

/// The next arm at `pos`: skip `;`s, then one `(pattern) body` up to
/// `done_idx`. Returns the arm and the position just past its body, or `None`
/// when only `;`s remain before `done`.
pub(super) fn next_arm(
    tokens: &[Token],
    text: &ScriptText,
    pos: usize,
    done_idx: usize,
) -> Result<Option<(WaitArm, usize)>, Report<ParseError>> {
    let mut pos = pos;
    while pos < done_idx
        && tokens
            .get(pos)
            .is_some_and(|(t, _, _, _, _)| t.eq_bytes(b";"))
    {
        pos += 1;
    }
    if pos >= done_idx {
        return Ok(None);
    }
    let pat_end = tokens
        .get(pos..done_idx)
        .and_then(|s| s.iter().position(|(t, _, _, _, _)| t.eq_bytes(b")")))
        .map(|i| pos + i)
        .ok_or(ParseError::WaitMissingCloseParen)?;
    let (pattern, captures) = pattern::parse_pattern(
        trim_semi(tokens.get(pos..pat_end).unwrap_or(&[])),
        text.start,
    )?;
    let (body, next) = extract::body(tokens, text, pat_end + 1, done_idx)?;
    Ok(Some((
        WaitArm {
            pattern,
            captures,
            body,
        },
        next,
    )))
}
