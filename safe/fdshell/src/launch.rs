#![forbid(unsafe_code)]

use crate::child::{self, Command};
use crate::parse::CommandLine;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub struct LaunchOutcome {
    pub pidfd: sys::LocalFd,
    pub capture_fd: Option<sys::LocalFd>,
    pub child_pid: i32,
}

pub fn launch(cell: &ForkCell<ShellState>, cmdline: &CommandLine) -> Result<LaunchOutcome, i32> {
    let cmd = Command::from(cmdline);

    let opened = crate::redirect::open_redirect_files(&cmdline.redirects)?;
    let resolved = {
        let state = cell.borrow_mut()?;
        crate::redirect::resolve_redirects(&cmdline.redirects, &opened, &state)?
    };

    let (capture_fd, child_fd) = if cmdline.captures.is_empty() {
        (None, None)
    } else {
        let (cap, ch) = sys::net::socketpair_with_passcred()?;
        (Some(cap), Some(ch))
    };
    let (child_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd_cell(cell)?;

    match pidfd_opt {
        None => child::child_exec(child_fd, cell, cmd, &cmdline.args, &resolved),
        Some(pidfd) => Ok(LaunchOutcome {
            pidfd,
            capture_fd,
            child_pid: child_pid as i32,
        }),
    }
}
