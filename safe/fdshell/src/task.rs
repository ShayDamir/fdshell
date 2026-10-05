mod reap;

use crate::capture::Capture;
use crate::error::task::TaskError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::Report;
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
            let task = state.tasks.remove(&key).ok_or(TaskError::NotFound)?;
            reap::reap(task, state)
        }
        None => reap::wait_all(state),
    }
}

/// POSIX `wait [pid…]`: reap the named pids (every task when no argument) and
/// return the last reaped status. A non-numeric argument is `BadPid`; an
/// unknown pid is `NotFound`.
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
            .map_err(|_e| TaskError::BadPid { arg: arg.clone() })?;
        let target = sys::Pid::from_raw(pid);
        let key = state
            .tasks
            .iter()
            .find(|(_, t)| t.child_pid == target)
            .map(|(k, _)| k.clone())
            .ok_or(TaskError::NotFound)?;
        let task = state.tasks.remove(&key).ok_or(TaskError::NotFound)?;
        last = reap::reap(task, state)?;
    }
    Ok(last)
}

#[cfg(test)]
mod tests;
