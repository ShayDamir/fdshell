#![forbid(unsafe_code)]

use crate::capture::Capture;
use crate::parse::{CommandLine, Pipeline};
use crate::task::Task;
use crate::vars::FdVars;
use std::collections::HashMap;
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

fn apply_captures(
    capture_fd: sys::LocalFd,
    child_pid: i32,
    captures: Vec<Capture>,
    fdvars: &mut FdVars,
) -> Result<(), i32> {
    let entries = crate::capture::do_captures(capture_fd, child_pid, captures, fdvars)?;
    for (var, fd) in entries {
        fdvars.insert(var, fd);
    }
    Ok(())
}

pub fn finish_cmd(
    cmdline: CommandLine,
    outcome: crate::launch::LaunchOutcome,
    fdvars: &mut FdVars,
    tasks: &mut HashMap<ShortCStr, Task>,
) -> Result<WaitStatus, i32> {
    match cmdline.pidvar {
        Some(name) => {
            tasks.insert(
                name,
                Task {
                    pidfd: outcome.pidfd,
                    capture_fd: outcome.capture_fd,
                    child_pid: outcome.child_pid,
                    captures: cmdline.captures,
                },
            );
            Ok(WaitStatus::Exited(0))
        }
        None => {
            let status = sys::wait_pidfd::wait_pidfd(&outcome.pidfd)?;
            if let WaitStatus::Exited(0) = status
                && let Some(capture_fd) = outcome.capture_fd
            {
                apply_captures(capture_fd, outcome.child_pid, cmdline.captures, fdvars)?;
            }
            Ok(status)
        }
    }
}

pub fn run_pipeline(pipeline: Pipeline, fdvars: &mut FdVars) -> Result<WaitStatus, i32> {
    let (status, channels) = crate::pipeline::launch_pipeline(fdvars, pipeline)?;
    if let WaitStatus::Exited(0) = status {
        for ch in channels {
            apply_captures(ch.capture_fd, ch.child_pid, ch.captures, fdvars)?;
        }
    }
    Ok(status)
}
