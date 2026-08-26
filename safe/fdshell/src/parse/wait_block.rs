mod pattern;

use super::Token;
use super::case_clause::extract;
use super::semi::trim_semi;
use crate::capture::Capture;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ensure};
use sys::{ScriptText, ShortCStr};

/// An event-case `wait` block: one poll round over fd variables.
#[cfg_attr(test, derive(Debug))]
pub struct WaitBlock {
    pub arms: Vec<WaitArm>,
}

#[cfg_attr(test, derive(Debug))]
pub struct WaitArm {
    pub pattern: WaitPattern,
    pub captures: Vec<Capture>,
    pub body: ScriptText,
}

/// What an arm waits for. `After` holds a millisecond deadline.
#[cfg_attr(test, derive(Debug))]
pub enum WaitPattern {
    Readable(FdRef),
    Writable(FdRef),
    Finished(FdRef),
    After(usize),
}

/// The fd a pattern waits on: a scalar var, an array wildcard, or a task pidfd.
#[cfg_attr(test, derive(Debug))]
pub enum FdRef {
    Var(ShortCStr),
    Array(ShortCStr),
    Task(ShortCStr),
}

pub(crate) fn tokens_to_wait(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<WaitBlock, Report<ParseError>> {
    ensure!(
        tokens
            .first()
            .is_some_and(|(t, _, _, _)| t.eq_bytes(b"wait")),
        ParseError::MalformedWaitBlock
    );
    ensure!(
        tokens
            .last()
            .is_some_and(|(t, _, _, _)| t.eq_bytes(b"done")),
        ParseError::ExpectedDone
    );

    let done_idx = tokens.len() - 1;
    let mut arms = Vec::new();
    let mut pos = 1;
    while pos < done_idx {
        if tokens.get(pos).is_some_and(|(t, _, _, _)| t.eq_bytes(b";")) {
            pos += 1;
            continue;
        }
        let pat_end = tokens
            .get(pos..done_idx)
            .and_then(|s| s.iter().position(|(t, _, _, _)| t.eq_bytes(b")")))
            .map(|i| pos + i)
            .ok_or(ParseError::WaitMissingCloseParen)?;
        let (pattern, captures) = pattern::parse_pattern(
            trim_semi(tokens.get(pos..pat_end).unwrap_or(&[])),
            text.start,
        )?;
        pos = pat_end + 1;
        let (body, next) = extract::body(tokens, text, pos, done_idx)?;
        arms.push(WaitArm {
            pattern,
            captures,
            body,
        });
        pos = next;
    }
    ensure!(!arms.is_empty(), ParseError::WaitEmptyBlock);
    Ok(WaitBlock { arms })
}
