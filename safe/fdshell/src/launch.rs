#![forbid(unsafe_code)]

use crate::child::{self, Command};
use crate::parse::CommandLine;
use crate::vars::FdVars;
use sys::siginfo::WaitStatus;

pub fn launch(vars: &FdVars, cmdline: &CommandLine) -> Result<(WaitStatus, sys::Fd), i32> {
    let cmd = if cmdline.builtin {
        Command::Builtin(cmdline.command.clone())
    } else {
        Command::External(cmdline.command.clone())
    };

    let (capture_fd, child_fd) = sys::net::socketpair()?;
    let (_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;

    match pidfd_opt {
        None => child::child_exec(child_fd, vars, cmd, &cmdline.args, &cmdline.redirects),
        Some(pidfd) => {
            let status = sys::wait_pidfd::wait_pidfd(&pidfd)?;
            Ok((status, capture_fd))
        }
    }
}
