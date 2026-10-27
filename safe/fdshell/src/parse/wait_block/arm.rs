//! One `wait` arm: the `(pattern)` head and its body.

use super::WaitArm;
use super::pattern;
use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::parse::case_clause::extract;
use crate::parse::semi::trim_semi;
use crate::parse::word_quoted::span_word;
use error_stack::Report;
use sys::ScriptText;

/// The next arm at `pos`: skip `;`s, then one `(pattern) body` up to
/// `done_idx`. Returns the arm and the position just past its body, or `None`
/// when only `;`s remain before `done`.
///
/// The skip loop needs no `done_idx` bound: `tokens[done_idx]` is the `done`
/// keyword (`block_tail::closer_index`, bare `done` or the redirection-glued
/// `done>f`), so it never matches the `;` needle and the walk stops by itself — a
/// bound there is a redundant ±1 (LESSONS: "Redundant boundary ±1 produces
/// equivalent mutants"). `pos ≤ done_idx` always (the caller starts at 1,
/// `extract::body` resumes at the `;;` pair's end, at most `done_idx`, or at
/// `done_idx`), so the bail below is the whole "only `;`s remain" contract; its
/// `>=` is load-bearing — with `>` a `;`-only tail is searched for a `)` and bails
/// `WaitMissingCloseParen` (pinned in `wait_block/tests.rs`).
pub(super) fn next_arm(
    tokens: &[Token],
    text: &ScriptText,
    pos: usize,
    done_idx: usize,
) -> Result<Option<(WaitArm, usize)>, Report<ParseError>> {
    let mut pos = pos;
    while tokens.get(pos).is_some_and(|tok| span_word(tok, b";")) {
        pos += 1;
    }
    if pos >= done_idx {
        return Ok(None);
    }
    let pat_end = tokens
        .get(pos..done_idx)
        .and_then(|s| s.iter().position(|tok| span_word(tok, b")")))
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
