use crate::capture::Capture;
use crate::error::task::TaskError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

pub struct Task {
    pub pidfd: sys::LocalFd,
    pub capture_fd: Option<sys::LocalFd>,
    pub child_pid: i32,
    pub captures: Vec<Capture>,
}

pub fn try_wait(
    args: &[ShortCStr],
    state: &mut ShellState,
) -> Result<WaitStatus, Report<TaskError>> {
    match args.first() {
        Some(arg) => {
            let key = arg.strip_prefix(b"&").ok_or(TaskError::BadArg)?;
            let Some(task) = state.tasks.remove(&key) else {
                return Err(Report::new(TaskError::NotFound));
            };
            let status =
                sys::wait_pidfd::wait_pidfd(&task.pidfd).change_context(TaskError::Wait)?;
            if let WaitStatus::Exited(0) = status
                && let Some(capture_fd) = task.capture_fd
            {
                let entries = match crate::capture::do_captures(
                    capture_fd,
                    task.child_pid,
                    task.captures,
                    state,
                ) {
                    Ok(v) => v,
                    Err(_) => return Err(Report::new(TaskError::Wait)),
                };
                for (var, fd) in entries {
                    state.fds.insert(var, fd);
                }
            }
            Ok(status)
        }
        None => {
            let mut last = WaitStatus::Exited(0);
            let keys: Vec<ShortCStr> = state.tasks.keys().cloned().collect();
            for key in keys {
                let Some(task) = state.tasks.remove(&key) else {
                    continue;
                };
                let status =
                    sys::wait_pidfd::wait_pidfd(&task.pidfd).change_context(TaskError::Wait)?;
                if let WaitStatus::Exited(0) = status
                    && let Some(capture_fd) = task.capture_fd
                    && let Ok(entries) = crate::capture::do_captures(
                        capture_fd,
                        task.child_pid,
                        task.captures,
                        state,
                    )
                {
                    for (var, fd) in entries {
                        state.fds.insert(var, fd);
                    }
                }
                last = status;
            }
            Ok(last)
        }
    }
}
