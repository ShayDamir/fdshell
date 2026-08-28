//! `timeout <seconds> <cmd> [args ...]` — run a command with a wall-clock deadline.
//!
//! Runs in-shell: the command is launched as a child and the shell polls the
//! child's pidfd against a one-shot deadline timer. On timeout the child is
//! sent SIGTERM, then SIGKILL if it survives a grace period, and the command
//! exits 124 (matching coreutils `timeout`).

use alloc::vec;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

mod parse;

pub(crate) fn run_timeout(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "timeout", cmdline)?;
    let cfg = parse::parse(&cmdline.args, &cmdline.args_mask)?;
    let subcmdline = crate::parse::CommandLine {
        builtin: false,
        command: cfg.command,
        args: cfg.args,
        args_mask: cfg.args_mask,
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    };
    let outcome =
        crate::launch::launch(cell, &subcmdline).change_context(CmdError::TimeoutLaunch)?;
    let exit = bounded_wait(&outcome.pidfd, cfg.seconds)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_exit(exit);
    Ok(true)
}

/// Poll the child's pidfd against a one-shot deadline timer. Returns the
/// child's exit code, or 124 if the deadline fired first (after SIGTERM, then
/// SIGKILL).
fn bounded_wait(pidfd: &sys::LocalFd, seconds: i64) -> Result<i32, Report<CmdError>> {
    let timer = sys::timerfd::timerfd_create(0).change_context(CmdError::TimeoutTimer)?;
    sys::timerfd::timerfd_settime(&timer, (seconds, 0), (0, 0))
        .change_context(CmdError::TimeoutTimer)?;
    let mut pfd = [
        sys::poll::PollFd::new(pidfd.as_raw(), sys::poll::POLLIN),
        sys::poll::PollFd::new(timer.as_raw(), sys::poll::POLLIN),
    ];
    // `poll(-1)` blocks until at least one fd is ready, so exactly one of the
    // two arms fired: the child's pidfd or the deadline timer.
    sys::poll::poll(&mut pfd, -1).change_context(CmdError::TimeoutPoll)?;
    let timed_out = pfd
        .get(1)
        .is_some_and(|p| p.revents & sys::poll::POLLIN != 0);
    if timed_out {
        sys::pidfd_send_signal::send_signal(pidfd, sys::pidfd_send_signal::SIGTERM)
            .change_context(CmdError::TimeoutSignal)?;
        // Grace period: give the child a chance to exit on SIGTERM.
        let mut grace = [sys::poll::PollFd::new(pidfd.as_raw(), sys::poll::POLLIN)];
        let n2 = sys::poll::poll(&mut grace, 1000).change_context(CmdError::TimeoutPoll)?;
        if n2 == 0 {
            sys::pidfd_send_signal::send_signal(pidfd, sys::pidfd_send_signal::SIGKILL)
                .change_context(CmdError::TimeoutSignal)?;
        }
        let _ = sys::wait_pidfd::wait_pidfd(pidfd).change_context(CmdError::TimeoutWait)?;
        Ok(124)
    } else {
        let status = sys::wait_pidfd::wait_pidfd(pidfd).change_context(CmdError::TimeoutWait)?;
        Ok(status.exit_code())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
