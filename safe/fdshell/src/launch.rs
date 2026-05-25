#![forbid(unsafe_code)]

use crate::child::{self, Command};
use crate::redirect::Redirect;
use crate::vars::Vars;
use std::ffi::CString;
use sys::siginfo::WaitStatus;

pub fn launch(
    vars: &Vars,
    cmd: Command,
    args: &[CString],
    redirects: &[Redirect],
) -> Result<(WaitStatus, sys::Fd), i32> {
    let (capture_fd, child_fd) = sys::net::socketpair()?;
    let (_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;

    match pidfd_opt {
        None => child::child_exec(child_fd, vars, cmd, args, redirects),
        Some(pidfd) => {
            let status = sys::wait_pidfd::wait_pidfd(&pidfd)?;
            Ok((status, capture_fd))
        }
    }
}
