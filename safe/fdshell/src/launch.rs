#![forbid(unsafe_code)]

use crate::child::{self, Command};
use crate::parse::CommandLine;
use crate::vars::FdVars;
use sys::siginfo::WaitStatus;

pub fn launch(
    vars: &FdVars,
    cmdline: &CommandLine,
) -> Result<(WaitStatus, Option<(sys::LocalFd, i32)>), i32> {
    let cmd = if cmdline.builtin {
        Command::Builtin(cmdline.command.clone())
    } else {
        Command::External(cmdline.command.clone())
    };

    let (capture_fd, child_fd) = if cmdline.captures.is_empty() {
        (None, None)
    } else {
        let (cap, ch) = sys::net::socketpair()?;
        sys::net::set_passcred(&cap)?;
        (Some(cap), Some(ch))
    };
    let (child_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;

    match pidfd_opt {
        None => child::child_exec(child_fd, vars, cmd, &cmdline.args, &cmdline.redirects),
        Some(pidfd) => {
            let status = sys::wait_pidfd::wait_pidfd(&pidfd)?;
            Ok((status, capture_fd.map(|fd| (fd, child_pid as i32))))
        }
    }
}
