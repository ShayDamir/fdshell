#![forbid(unsafe_code)]

use crate::capture::Capture;
use crate::vars::FdVars;
use std::collections::HashMap;
use sys::ShortCStr;
use sys::errno::EINVAL;
use sys::siginfo::WaitStatus;

pub struct Task {
    pub pidfd: sys::LocalFd,
    pub capture_fd: Option<sys::LocalFd>,
    pub child_pid: i32,
    pub captures: Vec<Capture>,
}

pub fn try_wait(
    args: &[ShortCStr],
    fdvars: &mut FdVars,
    tasks: &mut HashMap<ShortCStr, Task>,
) -> Result<WaitStatus, i32> {
    match args.first() {
        Some(arg) => {
            let key = arg.strip_prefix(b"&").ok_or(EINVAL)?;
            let Some(task) = tasks.remove(&key) else {
                return Err(EINVAL);
            };
            let status = sys::wait_pidfd::wait_pidfd(&task.pidfd)?;
            if let WaitStatus::Exited(0) = status
                && let Some(capture_fd) = task.capture_fd
            {
                let entries =
                    crate::capture::do_captures(capture_fd, task.child_pid, task.captures, fdvars)?;
                for (var, fd) in entries {
                    fdvars.insert(var, fd);
                }
            }
            Ok(status)
        }
        None => {
            let mut last = WaitStatus::Exited(0);
            let keys: Vec<ShortCStr> = tasks.keys().cloned().collect();
            for key in keys {
                let Some(task) = tasks.remove(&key) else {
                    continue;
                };
                let status = sys::wait_pidfd::wait_pidfd(&task.pidfd)?;
                if let WaitStatus::Exited(0) = status
                    && let Some(capture_fd) = task.capture_fd
                    && let Ok(entries) = crate::capture::do_captures(
                        capture_fd,
                        task.child_pid,
                        task.captures,
                        fdvars,
                    )
                {
                    for (var, fd) in entries {
                        fdvars.insert(var, fd);
                    }
                }
                last = status;
            }
            Ok(last)
        }
    }
}
