//! The condition phase of `parse/if_block.rs`: what sits between `if` and the
//! first `then` — its text slice, the heredoc bodies it claims, and its
//! background flag.

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::parse::cond_bodies::condition_bodies;
use crate::parse::semi::condition_background;
use crate::parse::semi::span_verbatim;
use crate::parse::semi::trim_condition;
use alloc::vec::Vec;
use error_stack::Report;
use sys::ScriptText;
use sys::ShortCStr;

/// The condition phase's result: the condition's own text slice, the heredoc
/// bodies it claims, and the plain-`&` background-job flag.
pub(super) struct Condition {
    pub(super) span: ScriptText,
    pub(super) bodies: Vec<ShortCStr>,
    pub(super) background: bool,
}

/// Build the condition of an `if` block from the tokens between `if` and the
/// first `then` (the closed slice from index 1 to `first_then`).
///
/// An `if`/`elif` condition is a compound list: `then` may sit behind the plain
/// `&` as behind a `;` (task #354), and that `&` is the condition's
/// background-job flag (task #356, `condition_background`).
///
/// The condition's emptiness rule (`if ; then`, `if & then`, task #299) lives in
/// `span_verbatim`'s `block_condition`, so it is checked before the `then` body's
/// `block_body` in the caller: `if ; then; fi` says `missing condition`.
pub(super) fn condition(
    closed: &[Token],
    text: &ScriptText,
    first_then: usize,
    line: &[u8],
) -> Result<Condition, Report<ParseError>> {
    let cond_slice = closed
        .get(1..first_then)
        .ok_or(ParseError::MissingCondition)?;
    let cond_tokens = trim_condition(cond_slice, line);
    let background = condition_background(cond_slice, line);

    let span = span_verbatim(
        text,
        closed,
        1,
        first_then,
        line,
        ParseError::MissingCondition,
    )?;
    let bodies = condition_bodies(text, cond_tokens);

    Ok(Condition {
        span,
        bodies,
        background,
    })
}
