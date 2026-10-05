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

/// Whether the `wait` tokens open an event-case block: the first non-`;`
/// token after `wait` is an unquoted word on a subsequent line, or a same-line
/// pattern keyword. A same-line pid/`$!`/name, a quoted word, or nothing is
/// the POSIX `wait` builtin (it falls through to a command).
pub(super) fn is_block(tokens: &[Token], text: &ScriptText) -> bool {
    let Some((_wt, _ws, we, _wq, _wm)) = tokens.first() else {
        return false;
    };
    let mut pos = 1;
    while let Some((t, _, _, _, _)) = tokens.get(pos) {
        if !t.eq_bytes(b";") {
            break;
        }
        pos += 1;
    }
    let Some((t, ts, _te, quoted, _tm)) = tokens.get(pos) else {
        return false;
    };
    if *quoted {
        return false;
    }
    let Some(word) = t.as_bytes().ok() else {
        return false;
    };
    let crossed = text
        .as_bytes()
        .ok()
        .and_then(|b| b.get(*we..*ts))
        .is_some_and(|between| between.contains(&b'\n'));
    crossed || crate::keywords::is_wait_pattern_keyword(word)
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
