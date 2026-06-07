use crate::child::{self, Command};
use crate::parse::CommandLine;
use crate::redirect::Redirect;
use crate::state::ShellState;
use sys::LocalFd;

pub fn run_child(
    i: usize,
    pipes: &[(LocalFd, LocalFd)],
    capture_pairs: &mut [Option<(LocalFd, LocalFd)>],
    commands: &[CommandLine],
    state: &ShellState,
) -> ! {
    let cmd_data = match commands.get(i) {
        Some(c) => c,
        None => std::process::exit(sys::errno::EINVAL),
    };

    let mut redirects: Vec<Redirect<'_>> = Vec::new();

    if let Some(prev) = i.checked_sub(1).and_then(|p| pipes.get(p)) {
        redirects.push(Redirect::new(0, &prev.0));
    }
    if let Some(wr) = pipes.get(i) {
        redirects.push(Redirect::new(1, &wr.1));
    }

    let opened = super::open::open_redirect_files(cmd_data);

    let file_redirects =
        match crate::redirect::resolve_redirects(&cmd_data.redirects, &opened, state) {
            Ok(fds) => fds,
            Err(e) => std::process::exit(e),
        };
    redirects.extend(file_redirects);

    let child_sock = capture_pairs
        .get_mut(i)
        .and_then(|p| p.take().map(|(_, ch)| ch));

    let cmd = Command::from(cmd_data);

    child::child_exec(child_sock, state, cmd, &cmd_data.args, &redirects)
}
