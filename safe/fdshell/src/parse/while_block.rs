use super::Token;
use crate::error::parse::ParseError;
use crate::parse::cond_bodies::condition_bodies;
use crate::parse::semi::verbatim;
use crate::parse::semi::{
    List, Term, block_body, block_condition, condition_background, ensure_before_closer,
    find_preceded_by, up_to_closing,
};
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
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
    /// The redirections written after the closing `done` (task #298,
    /// `parse::block_tail`); they apply to the whole loop.
    pub redirects: Vec<RedirectDef>,
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
    let closed = up_to_closing(tokens, line, List::Plain, ParseError::ExpectedDone)?;
    // The closing `done` is the truncated slice's last token.
    let done_idx = closed.len() - 1;
    let redirects =
        crate::parse::block_tail::redirects(b"done", tokens.get(done_idx..).unwrap_or(&[]))?;

    // A loop condition is a compound list, so its `do` may be preceded by the
    // plain `&` (task #354: `while : & do :; done`), and that `&` makes the
    // condition a background job, one per iteration (task #356).
    // `find_preceded_by` only scores a keyword preceded by a `;`/`&` token and
    // token 0 is the loop's own keyword, so `do_idx >= 2` is the minimum
    // reachable index — a `>= 2` guard there can never fire (task #299 deleted
    // the dead one: it let `while ; do` through, LESSONS "redundant boundary
    // ±1"). The emptiness test is on the **trimmed** slice, below.
    let do_idx =
        find_preceded_by(closed, 1, b"do", line, Term::SemiAmp).ok_or(ParseError::ExpectedDo)?;

    let cond_slice = closed.get(1..do_idx).ok_or(ParseError::ExpectedDo)?;
    // An empty condition (`while ; do`, `while & do`, `until ; do`: bash rc 2)
    // is true at run time, so the loop would run unbounded (task #299).
    let cond_tokens = block_condition(cond_slice, line, ParseError::ExpectedCondition)?;
    let cond_background = condition_background(cond_slice, line);
    let condition = verbatim(text, cond_tokens)?;
    let cond_bodies = condition_bodies(text, cond_tokens);

    // POSIX #2.9: the closing `done` stands behind a command-list terminator, or
    // behind the closer of a compound command that is the body's last statement
    // (`while false; do if :; then :; fi done`, task #301). Checked **after**
    // the condition, so `while ; do done` keeps `expected condition`, and
    // **before** the body, so `while true; do done` reports the missing `;`.
    ensure_before_closer(
        closed,
        line,
        done_idx,
        ParseError::ExpectedSemicolonBeforeDone,
    )?;

    let body = crate::parse::part_text::part_text(
        text,
        block_body(
            closed.get(do_idx + 1..done_idx).unwrap_or(&[]),
            line,
            ParseError::EmptyDoBody,
        )?,
    )?;

    Ok(LoopBlock {
        condition,
        body,
        cond_bodies,
        cond_background,
        redirects,
    })
}
