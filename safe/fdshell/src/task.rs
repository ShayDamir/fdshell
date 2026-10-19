mod reap;

use crate::capture::Capture;
use crate::error::task::TaskError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

pub struct Task {
    pub pidfd: sys::LocalFd,
    pub capture_fd: Option<sys::LocalFd>,
    pub child_pid: sys::Pid,
    pub captures: Vec<Capture>,
}

pub fn try_wait(
    args: &[ShortCStr],
    state: &mut ShellState,
) -> Result<WaitStatus, Report<TaskError>> {
    match args.first() {
        Some(arg) => {
            let key = arg.strip_prefix(b"&").ok_or(TaskError::BadArg)?;
            let task = state.jobs.remove_named(&key).ok_or(TaskError::NotFound)?;
            reap::reap(task, state)
        }
        None => reap::wait_all(state),
    }
}

/// POSIX `wait [pid…]`: reap the named pids (every job when no argument) and
/// return the last reaped status (POSIX: no operands → 0, see [`reap::wait_all`]).
/// A pid search covers the named jobs first, then the plain-`&` jobs
/// (`wait $!` of an async command — task #326). A non-numeric argument is
/// `BadPid`; an unknown pid is `NotFound`.
pub fn posix_wait(
    args: &[ShortCStr],
    state: &mut ShellState,
) -> Result<WaitStatus, Report<TaskError>> {
    if args.is_empty() {
        return reap::wait_all(state);
    }
    let mut last = WaitStatus::Exited(0);
    for arg in args {
        let pid: i32 = arg
            .parse()
            .change_context(TaskError::BadPid { arg: arg.clone() })?;
        let target = sys::Pid::from_raw(pid);
        let task = state
            .jobs
            .remove_by_pid(target)
            .ok_or(TaskError::NotFound)?;
        last = reap::reap(task, state)?;
    }
    Ok(last)
}

#[cfg(test)]
mod tests;
