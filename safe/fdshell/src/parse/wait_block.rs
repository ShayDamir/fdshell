mod arm;
mod fdref;
mod gate;
mod pattern;

#[cfg(test)]
mod tests;

pub(crate) use gate::is_block;

use super::Token;
use crate::capture::Capture;
use crate::error::parse::ParseError;
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use error_stack::{Report, ensure};
use sys::{ScriptText, ShortCStr};

/// An event-case `wait` block: one poll round over fd variables.
#[cfg_attr(test, derive(Debug))]
pub struct WaitBlock {
    pub arms: Vec<WaitArm>,
    /// The redirections written after the closing `done` (task #298,
    /// `parse::block_tail`); they apply to the whole round, as for every other
    /// keyword block (bash has no `wait` block twin).
    pub redirects: Vec<RedirectDef>,
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
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"wait")),
        ParseError::MalformedWaitBlock
    );
    // The closer is the block's last `done` keyword, not its last token: a
    // redirection written behind it belongs to the block (task #298), and in the
    // single-line spelling the here-doc bodies sit past it as word data.
    let done_idx =
        crate::parse::block_tail::closer_index(tokens, b"done").ok_or(ParseError::ExpectedDone)?;
    let redirects =
        crate::parse::block_tail::redirects(b"done", tokens.get(done_idx..).unwrap_or(&[]))?;
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
    Ok(WaitBlock { arms, redirects })
}
