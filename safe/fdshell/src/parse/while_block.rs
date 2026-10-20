use super::Token;
use crate::error::parse::ParseError;
use crate::parse::cond_bodies::condition_bodies;
use crate::parse::semi::{
    List, Term, block_body, condition_background, find_preceded_by, up_to_closing,
};
use crate::parse::semi::{trim_condition, verbatim};
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
    /// The condition was terminated by the plain `&`: it runs as a background
    /// job, one per condition evaluation (`condition_background`, task #356).
    pub cond_background: bool,
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
        .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(keyword))
    {
        return Err(ParseError::Never.into());
    }

    // A condition-position heredoc's bodies sit in the block text after the
    // closing `done`: the tokens after it are body data, not block structure.
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let tokens = up_to_closing(tokens, line, List::Plain, ParseError::ExpectedDone)?;
    // The closing `done` is the truncated slice's last token.
    let done_idx = tokens.len() - 1;

    // A loop condition is a compound list, so its `do` may be preceded by the
    // plain `&` (task #354: `while : & do :; done`), and that `&` makes the
    // condition a background job, one per iteration (task #356).
    let do_idx =
        find_preceded_by(tokens, 1, b"do", line, Term::SemiAmp).ok_or(ParseError::ExpectedDo)?;
    ensure!(do_idx >= 2, ParseError::ExpectedCondition);

    let cond_slice = tokens.get(1..do_idx).ok_or(ParseError::ExpectedDo)?;
    let cond_tokens = trim_condition(cond_slice, line);
    let cond_background = condition_background(cond_slice, line);
    let condition = verbatim(text, cond_tokens)?;
    let cond_bodies = condition_bodies(text, cond_tokens);

    let body = crate::parse::part_text::part_text(
        text,
        block_body(
            tokens.get(do_idx + 1..done_idx).unwrap_or(&[]),
            line,
            ParseError::EmptyDoBody,
        )?,
    )?;

    Ok(LoopBlock {
        condition,
        body,
        cond_bodies,
        cond_background,
    })
}
