use crate::child::{self, Command};
use crate::parse::CommandLine;
use crate::redirect::Redirect;
use sys::LocalFd;
use sys::fork_cell::ForkCell;

use crate::state::ShellState;

pub fn run_child(
    i: usize,
    pipes: &[(LocalFd, LocalFd)],
    capture_pairs: &mut [Option<(LocalFd, LocalFd)>],
    commands: &[CommandLine],
    cell: &ForkCell<ShellState>,
) -> ! {
    let cmd_data = match commands.get(i) {
        Some(c) => c,
        None => std::process::exit(sys::errno::EINVAL),
    };

    let mut redirects: Vec<Redirect> = Vec::new();

    if let Some(prev) = i.checked_sub(1).and_then(|p| pipes.get(p)) {
        match prev.0.try_clone() {
            Ok(fd) => redirects.push(Redirect::new(0, fd)),
            Err(e) => std::process::exit(e.into()),
        }
    }
    if let Some(wr) = pipes.get(i) {
        match wr.1.try_clone() {
            Ok(fd) => redirects.push(Redirect::new(1, fd)),
            Err(e) => std::process::exit(e.into()),
        }
    }

    let opened = super::open::open_redirect_files(cmd_data);

    let file_redirects = match cell.borrow() {
        Ok(state) => {
            match crate::redirect::resolve_redirects(&cmd_data.redirects, &opened, &state) {
                Ok(fds) => fds,
                Err(_) => std::process::exit(1),
            }
        }
        Err(_) => std::process::exit(1),
    };
    redirects.extend(file_redirects);

    let child_sock = capture_pairs
        .get_mut(i)
        .and_then(|p| p.take().map(|(_, ch)| ch));

    let cmd = Command::from(cmd_data);

    child::child_exec(child_sock, cell, cmd, &cmd_data.args, &redirects)
}
