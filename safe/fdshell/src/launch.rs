use core::fmt::Write;
use error_stack::{Report, ResultExt};

use crate::child::{self, Command};
use crate::parse::CommandLine;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub struct LaunchOutcome {
    pub pidfd: sys::LocalFd,
    pub capture_fd: Option<sys::LocalFd>,
    pub child_pid: sys::Pid,
}

pub fn launch(
    cell: &ForkCell<ShellState>,
    cmdline: &CommandLine,
) -> Result<LaunchOutcome, Report<crate::error::launch::LaunchError>> {
    let cmd = Command::from(cmdline);

    let opened = crate::redirect::open_redirect_files(&cmdline.redirects, cell)
        .change_context(crate::error::launch::LaunchError::Redirect)?;
    let resolved = crate::redirect::resolve_redirects(&cmdline.redirects, &opened, cell)
        .change_context(crate::error::launch::LaunchError::Redirect)?;

    prehash(&cmd, cell);

    // Foreground commands always get a capture socket so the child can report
    // its last expanded word for `$_`; background commands only need one for
    // captures (bash does not update `$_` for background commands).
    let (capture_fd, child_fd) = if cmdline.captures.is_empty() && cmdline.pidvar.is_some() {
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
            &cmdline.args_mask,
            &resolved,
        ) {
            Ok(code) => sys::exit(code),
            Err(report) => {
                let _ = writeln!(crate::io::Stderr, "{report:?}");
                sys::exit(report.current_context().exit_code());
            }
        },
        Some(pidfd) => Ok(LaunchOutcome {
            pidfd,
            capture_fd,
            child_pid,
        }),
    }
}

/// Best-effort: resolve an external command name (hash table, then PATH) and
/// store the result, so real runs populate the table like bash. A failure
/// here is not an error — the child re-resolves and reports it.
fn prehash(cmd: &Command, cell: &ForkCell<ShellState>) {
    if cmd.builtin || cmd.name.contains(b'/') {
        return;
    }
    let is_builtin = {
        let Ok(state) = cell.borrow() else {
            return;
        };
        child::dispatch::builtin_first(&cmd.name, &state)
    };
    if is_builtin {
        return;
    }
    let path = {
        let Ok(state) = cell.borrow() else {
            return;
        };
        crate::exec::resolve_path_str(&cmd.name, &state.hash_table).ok()
    };
    let Some(path) = path else {
        return;
    };
    let Ok(mut state) = cell.borrow_mut() else {
        return;
    };
    if state.hash_table.get(&cmd.name) != Some(&path) {
        state.hash_table.insert(cmd.name.clone(), path);
    }
}
