mod arm;
mod fdref;
mod pattern;

use super::Token;
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
            .is_some_and(|(t, _, _, _, _)| t.eq_bytes(b"wait")),
        ParseError::MalformedWaitBlock
    );
    ensure!(
        tokens
            .last()
            .is_some_and(|(t, _, _, _, _)| t.eq_bytes(b"done")),
        ParseError::ExpectedDone
    );

    let done_idx = tokens.len() - 1;
    let mut arms = Vec::new();
    let mut pos = 1;
    loop {
        let Some((arm, next)) = arm::next_arm(tokens, text, pos, done_idx)? else {
            break;
        };
        arms.push(arm);
        pos = next;
    }
    ensure!(!arms.is_empty(), ParseError::WaitEmptyBlock);
    Ok(WaitBlock { arms })
}
