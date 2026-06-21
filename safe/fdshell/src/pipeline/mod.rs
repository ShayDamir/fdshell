#![forbid(unsafe_code)]

mod child;
mod open;

use crate::capture::Capture;
use crate::error::pipeline::PipelineError;
use crate::parse::Pipeline;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

pub struct CaptureChannel {
    pub capture_fd: sys::LocalFd,
    pub child_pid: i32,
    pub captures: Vec<Capture>,
}

pub fn launch_pipeline(
    cell: &ForkCell<ShellState>,
    pipeline: Pipeline,
) -> Result<(WaitStatus, Vec<CaptureChannel>), Report<PipelineError>> {
    let n = pipeline.commands.len();
    let commands = pipeline.commands;

    let pipes = std::iter::repeat_with(|| sys::pipe::pipe2(sys::fcntl::O_CLOEXEC))
        .take(n.saturating_sub(1))
        .collect::<Result<Vec<_>, _>>()
        .change_context(PipelineError::Pipe)?;

    let mut capture_pairs = commands
        .iter()
        .map(|cmd| {
            if cmd.captures.is_empty() {
                Ok(None)
            } else {
                sys::net::socketpair_with_passcred().map(Some)
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .change_context(PipelineError::CaptureSocket)?;

    let mut children: Vec<(i32, sys::LocalFd)> = Vec::with_capacity(n);

    for i in 0..n {
        let (child_pid, pidfd_opt) =
            sys::fork_pidfd::fork_pidfd_cell(cell).change_context(PipelineError::Pipeline)?;
        match pidfd_opt {
            None => match child::run_child(i, &pipes, &mut capture_pairs, &commands, cell) {
                Ok(code) => std::process::exit(code),
                Err(report) => {
                    eprintln!("{:?}", report);
                    std::process::exit(report.current_context().exit_code());
                }
            },
            Some(pidfd) => children.push((child_pid as i32, pidfd)),
        }
    }

    drop(pipes);

    let channels: Vec<CaptureChannel> = (0..n)
        .filter_map(|i| {
            let pair = capture_pairs.get_mut(i)?;
            let (parent_end, _child_end) = pair.take()?;
            let (ch, cmd) = (children.get(i)?, commands.get(i)?);
            Some(CaptureChannel {
                capture_fd: parent_end,
                child_pid: ch.0,
                captures: cmd.captures.clone(),
            })
        })
        .collect();

    let last = children.last().ok_or(PipelineError::Pipeline)?;
    let last_status =
        sys::wait_pidfd::wait_pidfd(&last.1).change_context(PipelineError::Pipeline)?;

    for i in 0..n.saturating_sub(1) {
        if let Some(ch) = children.get(i) {
            let _ = sys::wait_pidfd::wait_pidfd(&ch.1);
        }
    }

    Ok((last_status, channels))
}
