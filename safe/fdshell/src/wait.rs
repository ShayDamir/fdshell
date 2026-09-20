mod arm;
pub(crate) mod round;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::error::wait::WaitError;
use crate::loop_control::LoopControl;
use crate::parse::wait_block::WaitBlock;
use crate::state::ShellState;

use arm::dispatch;
use round::build;

/// Run one `wait` round: poll the block's fds, dispatch a forked arm child per
/// ready descriptor (or the `after` arms on timeout), and apply keep/release.
pub(crate) fn run_wait(
    block: &WaitBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let mut pollset = build(block, cell).change_context(CmdError::Wait)?;
    let ready = poll_round(&mut pollset.entries, pollset.timeout).change_context(CmdError::Wait)?;
    let mut last = 0i32;
    let mut claimed: Vec<i32> = Vec::new();
    for e in pollset.entries.iter() {
        if e.revents & e.ready_mask == 0 {
            continue;
        }
        if claimed.contains(&e.raw) {
            continue;
        }
        claimed.push(e.raw);
        let arm = block.arms.get(e.arm).ok_or(CmdError::Never)?;
        last = dispatch(arm, Some(e.raw), &e.release, e.finished, cell)
            .change_context(CmdError::Wait)?;
    }
    if ready == 0 {
        for ai in pollset.after_arms.iter() {
            let arm = block.arms.get(*ai).ok_or(CmdError::Never)?;
            let none = round::ReleaseKey::None;
            last = dispatch(arm, None, &none, false, cell).change_context(CmdError::Wait)?;
        }
    }
    let mut s = cell.borrow_mut().change_context(CmdError::Never)?;
    s.set_last_exit(last);
    Ok(None)
}

/// Poll the round's descriptors once and record each descriptor's `revents`.
fn poll_round(entries: &mut [round::PollEntry], timeout: i32) -> Result<usize, Report<WaitError>> {
    let mut fds: Vec<sys::poll::PollFd> = entries
        .iter()
        .map(|e| sys::poll::PollFd::new(e.raw, e.events))
        .collect();
    let n = sys::poll::poll(&mut fds, timeout).change_context(WaitError::Poll)?;
    for (fd, e) in fds.iter().zip(entries.iter_mut()) {
        e.revents = fd.revents;
    }
    Ok(n)
}

#[cfg(test)]
mod tests;
