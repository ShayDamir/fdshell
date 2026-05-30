#![forbid(unsafe_code)]

mod child;
mod open;

use crate::capture::Capture;
use crate::parse::Pipeline;
use crate::vars::FdVars;
use sys::siginfo::WaitStatus;

pub struct CaptureChannel {
    pub capture_fd: sys::LocalFd,
    pub child_pid: i32,
    pub captures: Vec<Capture>,
}

pub fn launch_pipeline(
    vars: &FdVars,
    pipeline: Pipeline,
) -> Result<(WaitStatus, Vec<CaptureChannel>), i32> {
    let n = pipeline.commands.len();
    let commands = pipeline.commands;

    let mut pipes: Vec<(sys::LocalFd, sys::LocalFd)> = Vec::with_capacity(n.saturating_sub(1));
    for _ in 0..n.saturating_sub(1) {
        let (rd, wr) = sys::pipe::pipe2(sys::fcntl::O_CLOEXEC)?;
        pipes.push((rd, wr));
    }

    let mut capture_pairs: Vec<Option<(sys::LocalFd, sys::LocalFd)>> = Vec::with_capacity(n);
    for cmd in &commands {
        if cmd.captures.is_empty() {
            capture_pairs.push(None);
        } else {
            let (parent, child) = sys::net::socketpair()?;
            sys::net::set_passcred(&parent)?;
            capture_pairs.push(Some((parent, child)));
        }
    }

    let mut children: Vec<(i32, sys::LocalFd)> = Vec::with_capacity(n);

    for i in 0..n {
        let (child_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
        match pidfd_opt {
            None => child::run_child(i, &pipes, &mut capture_pairs, &commands, vars),
            Some(pidfd) => children.push((child_pid as i32, pidfd)),
        }
    }

    drop(pipes);

    let mut channels: Vec<CaptureChannel> = Vec::new();
    for i in 0..n {
        if let Some(pair) = capture_pairs.get_mut(i)
            && let Some((parent_end, child_end)) = pair.take()
        {
            drop(child_end);
            if let (Some(ch), Some(cmd)) = (children.get(i), commands.get(i)) {
                channels.push(CaptureChannel {
                    capture_fd: parent_end,
                    child_pid: ch.0,
                    captures: cmd.captures.clone(),
                });
            }
        }
    }
    drop(capture_pairs);
    drop(commands);

    let last = children.last().ok_or(sys::errno::EINVAL)?;
    let last_status = sys::wait_pidfd::wait_pidfd(&last.1)?;

    for i in 0..n.saturating_sub(1) {
        if let Some(ch) = children.get(i) {
            let _ = sys::wait_pidfd::wait_pidfd(&ch.1);
        }
    }

    Ok((last_status, channels))
}
