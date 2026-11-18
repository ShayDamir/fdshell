// mutants: -C --lib -C --test -C wait -C --test -C trap -- -E 'test(/^wait::/) | binary(wait) | binary(trap)'

mod arm;
pub(crate) mod round;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::error::wait::WaitError;
use crate::loop_control::LoopControl;
use crate::parse::wait_block::WaitBlock;
use crate::run::failing;
use crate::state::ShellState;

use arm::dispatch;
use round::build;

/// Run one `wait` round: poll the block's fds, dispatch a forked arm child per
/// ready descriptor (or the `after` arms on timeout), and apply keep/release.
pub(crate) fn run_wait(
    block: &WaitBlock,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    // The redirections written after the block's `done` apply to the whole poll
    // round (task #298 — family consistency: bash has no `wait` block twin). A
    // failing open lands as the round's status 1 and the script continues
    // (task #179).
    let scope = match crate::redirect::Scope::open(&block.redirects, cell) {
        Ok(scope) => scope,
        Err(report) => {
            failing::convert(report, cell)?;
            return Ok(None);
        }
    };
    scope.run(|| run_round(block, cell))
}

/// The round proper: poll the block's fds, dispatch a forked arm child per ready
/// descriptor (or the `after` arms on timeout), and apply keep/release.
fn run_round(
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
    // Boundary after the round's polls and arm dispatches; the plan places it
    // before `set_last_exit`, and the round now returns what a running action
    // raises (a `break` inside an action reaches the block's own loop level).
    let control = crate::trap_exec::run_pending(cell)?;
    let mut s = cell.borrow_mut().change_context(CmdError::Never)?;
    s.set_last_exit(last);
    Ok(control)
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
