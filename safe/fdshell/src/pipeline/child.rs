use crate::child::{self, Command};
use crate::error::child::ChildError;
use crate::parse::CommandLine;
use crate::redirect::Redirect;
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
) -> Result<i32, Report<ChildError>> {
    let cmd_data = commands
        .get(i)
        .ok_or_else(|| Report::new(ChildError::ExecFailed))?;

    let mut redirects: Vec<Redirect> = Vec::new();

    if let Some(prev) = i.checked_sub(1).and_then(|p| pipes.get(p)) {
        let fd = prev
            .0
            .try_clone()
            .change_context(ChildError::RedirectFailed)?;
        redirects.push(Redirect::new(0, fd));
    }
    if let Some(wr) = pipes.get(i) {
        let fd =
            wr.1.try_clone()
                .change_context(ChildError::RedirectFailed)?;
        redirects.push(Redirect::new(1, fd));
    }

    let opened =
        super::open::open_redirect_files(cmd_data).change_context(ChildError::RedirectFailed)?;

    let file_redirects = {
        let state = cell.borrow().change_context(ChildError::BorrowFailed)?;
        crate::redirect::resolve_redirects(&cmd_data.redirects, &opened, &state)
            .change_context(ChildError::RedirectFailed)?
    };
    redirects.extend(file_redirects);

    let child_sock = capture_pairs
        .get_mut(i)
        .and_then(|p| p.take().map(|(_, ch)| ch));

    let cmd = Command::from(cmd_data);

    child::child_main(child_sock, cell, cmd, &cmd_data.args, &redirects)
}
