#![forbid(unsafe_code)]

mod child;
mod open;

use crate::capture::Capture;
use crate::parse::Pipeline;
use crate::state::ShellState;
use sys::siginfo::WaitStatus;

pub struct CaptureChannel {
    pub capture_fd: sys::LocalFd,
    pub child_pid: i32,
    pub captures: Vec<Capture>,
}

pub fn launch_pipeline(
    state: &ShellState,
    pipeline: Pipeline,
) -> Result<(WaitStatus, Vec<CaptureChannel>), i32> {
    let n = pipeline.commands.len();
    let commands = pipeline.commands;

    let pipes = std::iter::repeat_with(|| sys::pipe::pipe2(sys::fcntl::O_CLOEXEC))
        .take(n.saturating_sub(1))
        .collect::<Result<Vec<_>, _>>()?;

    let mut capture_pairs = commands
        .iter()
        .map(|cmd| {
            if cmd.captures.is_empty() {
                Ok(None)
            } else {
                sys::net::socketpair_with_passcred().map(Some)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut children: Vec<(i32, sys::LocalFd)> = Vec::with_capacity(n);

    for i in 0..n {
        let (child_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
        match pidfd_opt {
            None => child::run_child(i, &pipes, &mut capture_pairs, &commands, state),
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

    let last = children.last().ok_or(sys::errno::EINVAL)?;
    let last_status = sys::wait_pidfd::wait_pidfd(&last.1)?;

    for i in 0..n.saturating_sub(1) {
        if let Some(ch) = children.get(i) {
            let _ = sys::wait_pidfd::wait_pidfd(&ch.1);
        }
    }

    Ok((last_status, channels))
}
