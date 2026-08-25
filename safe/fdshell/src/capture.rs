use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};

use crate::error::capture::CaptureError;
use crate::state::ShellState;
use sys::{LocalFd, Position, ShortCStr};

mod commit;
mod slot;

pub use commit::{CapturedFd, capture_and_commit};

#[cfg(test)]
mod tests;

// Clone required by pipeline/mod.rs (cmd.captures.clone()).
// Debug + PartialEq are test-only — quarantined behind cfg_attr.
#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Capture {
    pub var: ShortCStr,
    pub tag: Option<ShortCStr>,
    pub force: bool,
    /// Bounded capture: append at most this many entries to the array var.
    pub cap: Option<usize>,
    /// Position of the capture token in the source line.
    pub set_at: Position,
}

/// Receive fds from `capture_fd`, match them against captures, stage results.
///
/// Unbounded captures take exactly one fd; bounded ones (`cap: Some(n)`)
/// append to an array var up to `n` entries in total, closing excess fds.
/// Commit the result with `commit_captured`.
pub fn do_captures(
    capture_fd: LocalFd,
    expected_pid: sys::Pid,
    captures: Vec<Capture>,
    state: &ShellState,
) -> Result<Vec<CapturedFd>, Report<CaptureError>> {
    let mut slots = captures
        .into_iter()
        .map(|c| slot::Slot::new(c, state))
        .collect::<Result<Vec<_>, _>>()?;

    while slots.iter().any(slot::Slot::needs_more) {
        let mut buf = [0u8; sys::shellfd::TAG_MAX];
        let (fd, rtag) = match sys::shellfd::recv_fd(&capture_fd, &mut buf, expected_pid) {
            Ok(v) => v,
            Err(e) => {
                let ctx = e.current_context();
                if matches!(*ctx, sys::RecvFdError::Closed) {
                    break;
                }
                return Err(e).change_context(CaptureError::ReceiveFailed);
            }
        };
        if crate::last_arg::is_tag(rtag) {
            continue;
        }
        let idx = slots.iter().position(|s| s.needs_more() && s.matches(rtag));
        if let Some(i) = idx
            && let Some(slot) = slots.get_mut(i)
        {
            slot.take(fd, rtag);
        }
    }

    let unmet = slots.iter().filter(|s| !s.satisfied()).count();
    if unmet > 0 {
        let received = slots.iter().map(|s| s.entries.len()).sum::<usize>();
        bail!(CaptureError::Incomplete {
            expected: unmet + received,
            received,
        });
    }
    let mut out = Vec::with_capacity(slots.len());
    for slot in slots {
        out.push(slot.finish()?);
    }
    Ok(out)
}
