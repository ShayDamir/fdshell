#![forbid(unsafe_code)]

use crate::child::{self, Command};
use crate::parse::CommandLine;
use crate::redirect::{Redirect, RedirectSource};
use crate::vars::FdVars;
use sys::fcntl::O_CLOEXEC;
use sys::siginfo::WaitStatus;

pub fn launch(
    vars: &FdVars,
    cmdline: &CommandLine,
) -> Result<(WaitStatus, Option<(sys::LocalFd, i32)>), i32> {
    let cmd = Command::from(cmdline);

    let mut opened: Vec<sys::LocalFd> = Vec::with_capacity(cmdline.redirects.len());
    for r in &cmdline.redirects {
        if let RedirectSource::Path(path) = &r.source {
            let flags = r.direction.open_flags();
            let name = path.to_c_string();
            let fd = sys::openat2::openat2(
                sys::atfd::AtFd::cwd(),
                &name,
                &sys::openat2::OpenHow::new((flags | O_CLOEXEC) as u64, 0o666),
            )?;
            opened.push(fd);
        }
    }

    let mut resolved: Vec<Redirect<'_>> = Vec::with_capacity(cmdline.redirects.len());
    let mut path_idx = 0usize;
    for r in &cmdline.redirects {
        let local = match &r.source {
            RedirectSource::Var(var) => vars.resolve(var.as_bytes()).ok_or(sys::errno::EINVAL)?,
            RedirectSource::Path(_) => {
                let fd = opened.get(path_idx).ok_or(sys::errno::EIO)?;
                path_idx += 1;
                fd
            }
        };
        resolved.push(r.resolve(local));
    }

    let (capture_fd, child_fd) = if cmdline.captures.is_empty() {
        (None, None)
    } else {
        let (cap, ch) = sys::net::socketpair()?;
        sys::net::set_passcred(&cap)?;
        (Some(cap), Some(ch))
    };
    let (child_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;

    match pidfd_opt {
        None => child::child_exec(child_fd, vars, cmd, &cmdline.args, &resolved),
        Some(pidfd) => {
            let status = sys::wait_pidfd::wait_pidfd(&pidfd)?;
            Ok((status, capture_fd.map(|fd| (fd, child_pid as i32))))
        }
    }
}
