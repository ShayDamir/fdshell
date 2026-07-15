use crate::child::{self, Command};
use crate::error::child_process::ChildProcessError;
use crate::parse::CommandLine;
use crate::redirect::Redirect;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::LocalFd;
use sys::fork_cell::ForkCell;

use crate::state::ShellState;

pub fn run_child(
    i: usize,
    pipes: &[(LocalFd, LocalFd)],
    capture_pairs: &mut [Option<(LocalFd, LocalFd)>],
    commands: &[CommandLine],
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<ChildProcessError>> {
    let cmd_data = commands.get(i).ok_or(ChildProcessError::ExecFailed)?;

    let mut redirects: Vec<Redirect> = Vec::new();

    // Clone needed pipe fds for stdin/stdout redirects, then close ALL original
    // pipe fds. Each child inherits all pipe fds from the parent; we clone only
    // the ones this command needs and redirect to them. Closing all originals
    // ensures proper EOF signaling when writers exit.
    for (j, (read_end, write_end)) in pipes.iter().enumerate() {
        if j == i.saturating_sub(1) {
            let fd = read_end
                .try_clone()
                .change_context(ChildProcessError::RedirectFailed)?;
            redirects.push(Redirect::new(0, fd));
        }
        if j == i {
            let fd = write_end
                .try_clone()
                .change_context(ChildProcessError::RedirectFailed)?;
            redirects.push(Redirect::new(1, fd));
        }
    }
    // Close ALL original pipe fds — the clones above are what we use via redirects.
    for (read_end, write_end) in pipes.iter() {
        sys::close_raw(read_end.as_raw());
        sys::close_raw(write_end.as_raw());
    }

    let opened = crate::redirect::open_redirect_files(&cmd_data.redirects)
        .change_context(ChildProcessError::RedirectFailed)?;

    let file_redirects = {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::BorrowFailed)?;
        crate::redirect::resolve_redirects(&cmd_data.redirects, &opened, &state)
            .change_context(ChildProcessError::RedirectFailed)?
    };
    redirects.extend(file_redirects);

    let child_sock = capture_pairs
        .get_mut(i)
        .and_then(|p| p.take().map(|(_, ch)| ch));

    let cmd = Command::from(cmd_data);

    child::child_main(
        child_sock,
        cell,
        cmd,
        &cmd_data.args,
        &cmd_data.args_fq,
        &redirects,
    )
}
