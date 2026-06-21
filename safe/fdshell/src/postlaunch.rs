#![forbid(unsafe_code)]

use error_stack::{Report, ResultExt};

use crate::capture::Capture;
use crate::error::launch::LaunchError;
use crate::error::pipeline::PipelineError;
use crate::parse::{CommandLine, Pipeline};
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

fn apply_captures(
    capture_fd: sys::LocalFd,
    child_pid: i32,
    captures: Vec<Capture>,
    state: &mut ShellState,
) -> Result<(), crate::error::capture::CaptureError> {
    let entries = crate::capture::do_captures(capture_fd, child_pid, captures, state)?;
    for (var, fd) in entries {
        state.fds.insert(var, fd);
    }
    Ok(())
}

pub fn finish_cmd(
    cmdline: CommandLine,
    outcome: crate::launch::LaunchOutcome,
    state: &mut ShellState,
) -> Result<WaitStatus, Report<LaunchError>> {
    match cmdline.pidvar {
        Some(name) => {
            state.last_bg_pid = Some(outcome.child_pid);
            state.tasks.insert(
                name,
                crate::task::Task {
                    pidfd: outcome.pidfd,
                    capture_fd: outcome.capture_fd,
                    child_pid: outcome.child_pid,
                    captures: cmdline.captures,
                },
            );
            Ok(WaitStatus::Exited(0))
        }
        None => {
            let status =
                sys::wait_pidfd::wait_pidfd(&outcome.pidfd).change_context(LaunchError::Fork)?;
            if let WaitStatus::Exited(0) = status
                && let Some(capture_fd) = outcome.capture_fd
            {
                apply_captures(capture_fd, outcome.child_pid, cmdline.captures, state)
                    .change_context(LaunchError::CaptureSocket)?;
            }
            Ok(status)
        }
    }
}

pub fn run_pipeline(
    pipeline: Pipeline,
    cell: &ForkCell<ShellState>,
) -> Result<WaitStatus, Report<PipelineError>> {
    let (status, channels) = crate::pipeline::launch_pipeline(cell, pipeline)?;
    if let WaitStatus::Exited(0) = status {
        let mut state = cell.borrow_mut().change_context(PipelineError::Pipeline)?;
        for ch in channels {
            apply_captures(ch.capture_fd, ch.child_pid, ch.captures, &mut state)
                .change_context(PipelineError::Pipeline)?;
        }
    }
    Ok(status)
}
