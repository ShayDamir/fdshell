//! The deadline poll of `timeout`: the child's pidfd against a one-shot
//! deadline timer, and the SIGTERM → grace → SIGKILL sequence a fired deadline
//! triggers. The 124 exit code of coreutils `timeout` is decided here, by the
//! arm that fired.

use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;

/// Poll the child's pidfd against a one-shot deadline timer. Returns the
/// child's exit code (or 124 if the deadline fired first, after [`terminate`]
/// ran) plus the child's CPU times for `times` accounting.
pub(super) fn bounded_wait(
    pidfd: &sys::LocalFd,
    seconds: i64,
) -> Result<(i32, sys::getrusage::CpuTimes), Report<CmdError>> {
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
        terminate(pidfd)?;
        let (_, times) = pidfd
            .wait_pidfd_rusage()
            .change_context(CmdError::TimeoutWait)?;
        Ok((124, times))
    } else {
        let (status, times) = pidfd
            .wait_pidfd_rusage()
            .change_context(CmdError::TimeoutWait)?;
        Ok((status.exit_code(), times))
    }
}

/// The terminate sequence of a fired deadline: SIGTERM first, then SIGKILL if
/// the child is still alive after the one-second grace poll. The grace period
/// is the whole rule — a child that exits on SIGTERM is never killed by force,
/// and the reaping `wait_pidfd_rusage` runs only after the sequence.
fn terminate(pidfd: &sys::LocalFd) -> Result<(), Report<CmdError>> {
    pidfd
        .send_signal(sys::signal::SIGTERM)
        .change_context(CmdError::TimeoutSignal)?;
    // Grace period: give the child a chance to exit on SIGTERM.
    let mut grace = [sys::poll::PollFd::new(pidfd.as_raw(), sys::poll::POLLIN)];
    let n2 = sys::poll::poll(&mut grace, 1000).change_context(CmdError::TimeoutPoll)?;
    if n2 == 0 {
        pidfd
            .send_signal(sys::signal::SIGKILL)
            .change_context(CmdError::TimeoutSignal)?;
    }
    Ok(())
}
