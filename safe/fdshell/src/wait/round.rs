mod resolve;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::wait::WaitError;
use crate::parse::wait_block::{WaitBlock, WaitPattern};
use crate::state::ShellState;

/// What to release (close) a polled fd as when its arm does not keep it.
pub(crate) enum ReleaseKey {
    None,
    Var(ShortCStr),
    Array { arr: ShortCStr, source: ShortCStr },
    Task(ShortCStr),
}

/// One descriptor to poll, tied to the arm that requested it.
pub(crate) struct PollEntry {
    pub raw: i32,
    pub events: i16,
    pub revents: i16,
    pub ready_mask: i16,
    pub arm: usize,
    pub release: ReleaseKey,
    pub finished: bool,
}

/// The resolved descriptors, `after` arms, and deadline of one poll round.
pub(crate) struct Round {
    pub entries: Vec<PollEntry>,
    pub after_arms: Vec<usize>,
    pub timeout: i32,
}

/// The readiness an arm waits for.
enum Kind {
    Readable,
    Writable,
    Finished,
}

/// The requested and "ready" event bits for a kind (and whether the target is a pidfd).
fn events(kind: Kind, is_task: bool) -> (i16, i16) {
    use sys::poll::*;
    match kind {
        Kind::Readable => (POLLIN, POLLIN + POLLERR + POLLHUP + POLLNVAL),
        Kind::Writable => (POLLOUT, POLLOUT + POLLERR + POLLHUP + POLLNVAL),
        Kind::Finished => {
            if is_task {
                (POLLIN, POLLIN)
            } else {
                (POLLRDHUP + POLLHUP, POLLRDHUP + POLLHUP)
            }
        }
    }
}

/// Resolve every arm into the round's poll set, `after` arms and deadline.
pub(crate) fn build(
    block: &WaitBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Round, Report<WaitError>> {
    let state = cell.borrow().change_context(WaitError::Never)?;
    let mut entries = Vec::new();
    let mut after_arms = Vec::new();
    let mut timeout_ms: Option<usize> = None;
    for (ai, arm) in block.arms.iter().enumerate() {
        match &arm.pattern {
            WaitPattern::After(ms) => {
                after_arms.push(ai);
                timeout_ms = Some(match timeout_ms {
                    Some(cur) => cur.min(*ms),
                    None => *ms,
                });
            }
            WaitPattern::Readable(r) => {
                entries.extend(resolve::resolve(r, ai, Kind::Readable, &state)?)
            }
            WaitPattern::Writable(r) => {
                entries.extend(resolve::resolve(r, ai, Kind::Writable, &state)?)
            }
            WaitPattern::Finished(r) => {
                entries.extend(resolve::resolve(r, ai, Kind::Finished, &state)?)
            }
        }
    }
    if entries.is_empty() && after_arms.is_empty() {
        bail!(WaitError::EmptyPoll);
    }
    let timeout = timeout_ms
        .map(|ms| i32::try_from(ms).unwrap_or(i32::MAX))
        .unwrap_or(-1);
    Ok(Round {
        entries,
        after_arms,
        timeout,
    })
}

#[cfg(test)]
mod tests;
