use crate::child::{self, Command};
use crate::error::child_process::ChildProcessError;
use crate::parse::CommandLine;
use crate::redirect::Redirect;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{LocalFd, Pid};

use crate::state::ShellState;

mod close_inherited;

pub fn run_child(
    i: usize,
    pipes: &[(LocalFd, LocalFd)],
    capture_pairs: &mut [Option<(LocalFd, LocalFd)>],
    children: &[(Pid, LocalFd)],
    commands: &[CommandLine],
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<ChildProcessError>> {
    let cmd_data = commands.get(i).ok_or(ChildProcessError::ExecFailed)?;

    let mut redirects: Vec<Redirect> = Vec::new();

    // Clone the pipe ends this stage needs for its stdin/stdout redirects.
    // The clones use try_clone() (lowest free fd), which relies on the
    // inherited pipe ends still occupying the low fd numbers, so the
    // close_inherited cleanup runs after this loop.
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

    // Builtin stages never exec, so the inherited pipe ends, sibling
    // capture pairs and sibling pidfds would stay open for the whole
    // builtin run; close them here (external stages shed them at exec).
    close_inherited::close_inherited(pipes, capture_pairs, children, i)?;

    let opened = crate::redirect::open_redirect_files(&cmd_data.redirects, cell)
        .change_context(ChildProcessError::RedirectFailed)?;

    let file_redirects = crate::redirect::resolve_redirects(&cmd_data.redirects, &opened, cell)
        .change_context(ChildProcessError::RedirectFailed)?;
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
        &cmd_data.args_mask,
        &redirects,
    )
}
