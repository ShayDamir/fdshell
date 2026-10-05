use super::Token;
use crate::error::parse::ParseError;
use crate::parse::cond_bodies::condition_bodies;
use crate::parse::semi::{closing_keyword_index, find_preceded_by_semi, trim_semi, verbatim};
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, ensure};
use sys::ScriptText;
use sys::ShortCStr;

#[cfg_attr(test, derive(Debug))]
pub struct LoopBlock {
    pub condition: ScriptText,
    pub body: ScriptText,
    /// The condition's heredoc bodies (in operator order; empty when none).
    pub cond_bodies: Vec<ShortCStr>,
}

pub type WhileBlock = LoopBlock;
pub type UntilBlock = LoopBlock;

pub(crate) fn tokens_to_loop(
    tokens: &[Token],
    keyword: &[u8],
    text: &ScriptText,
) -> Result<LoopBlock, Report<ParseError>> {
    if !tokens
        .first()
        .is_some_and(|(t, _, _, _, _)| t.eq_bytes(keyword))
    {
        return Err(ParseError::Never.into());
    }

    // A condition-position heredoc's bodies sit in the block text after the
    // closing `done`: the tokens after it are body data, not block structure.
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let done_idx = closing_keyword_index(tokens, line).ok_or(ParseError::ExpectedDone)?;
    let tokens = tokens.get(..=done_idx).ok_or(ParseError::ExpectedDone)?;

    let do_idx = find_preceded_by_semi(tokens, 1, b"do").ok_or(ParseError::ExpectedDo)?;
    ensure!(do_idx >= 2, ParseError::ExpectedCondition);

    let cond_tokens = trim_semi(tokens.get(1..do_idx).ok_or(ParseError::ExpectedDo)?);
    let condition = verbatim(text, cond_tokens)?;
    let cond_bodies = condition_bodies(text, cond_tokens);

    let body = verbatim(
        text,
        trim_semi(
            tokens
                .get(do_idx + 1..done_idx)
                .ok_or(ParseError::ExpectedDone)?,
        ),
    )?;

    Ok(LoopBlock {
        condition,
        body,
        cond_bodies,
    })
}
