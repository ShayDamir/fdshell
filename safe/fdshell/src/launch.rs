#![forbid(unsafe_code)]

use error_stack::{Report, ResultExt};

use crate::child::{self, Command};
use crate::parse::CommandLine;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub struct LaunchOutcome {
    pub pidfd: sys::LocalFd,
    pub capture_fd: Option<sys::LocalFd>,
    pub child_pid: i32,
}

pub fn launch(
    cell: &ForkCell<ShellState>,
    cmdline: &CommandLine,
) -> Result<LaunchOutcome, Report<crate::error::launch::LaunchError>> {
    let cmd = Command::from(cmdline);

    let opened = crate::redirect::open_redirect_files(&cmdline.redirects)
        .change_context(crate::error::launch::LaunchError::Redirect)?;
    let resolved = {
        let state = cell
            .borrow_mut()
            .change_context(crate::error::launch::LaunchError::Borrow)?;
        crate::redirect::resolve_redirects(&cmdline.redirects, &opened, &state)
            .change_context(crate::error::launch::LaunchError::Redirect)?
    };

    let (capture_fd, child_fd) = if cmdline.captures.is_empty() {
        (None, None)
    } else {
        let (cap, ch) = sys::net::socketpair_with_passcred()
            .change_context(crate::error::launch::LaunchError::CaptureSocket)?;
        (Some(cap), Some(ch))
    };
    let (child_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd_cell(cell)
        .change_context(crate::error::launch::LaunchError::Fork)?;

    match pidfd_opt {
        None => match child::child_main(
            child_fd,
            cell,
            cmd,
            &cmdline.args,
            &cmdline.args_fq,
            &resolved,
        ) {
            Ok(code) => std::process::exit(code),
            Err(report) => {
                eprintln!("{:?}", report);
                std::process::exit(report.current_context().exit_code());
            }
        },
        Some(pidfd) => Ok(LaunchOutcome {
            pidfd,
            capture_fd,
            child_pid: child_pid as i32,
        }),
    }
}
