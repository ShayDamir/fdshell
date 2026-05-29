use crate::child::{self, Command};
use crate::parse::CommandLine;
use crate::vars::FdVars;
use sys::LocalFd;

pub fn run_child(
    i: usize,
    pipes: &[(LocalFd, LocalFd)],
    capture_pairs: &mut [Option<(LocalFd, LocalFd)>],
    commands: &[CommandLine],
    vars: &FdVars,
) -> ! {
    if let Some(prev) = i.checked_sub(1).and_then(|p| pipes.get(p))
        && let Err(e) = prev.0.export_to(0)
    {
        std::process::exit(e);
    }
    if let Some(wr) = pipes.get(i)
        && let Err(e) = wr.1.export_to(1)
    {
        std::process::exit(e);
    }

    let child_sock = match capture_pairs.get_mut(i) {
        Some(pair) => pair.take().map(|(_, ch)| ch),
        None => None,
    };

    let cmd_data = match commands.get(i) {
        Some(c) => c,
        None => std::process::exit(sys::errno::EINVAL),
    };
    let cmd = if cmd_data.builtin {
        Command::Builtin(cmd_data.command.clone())
    } else {
        Command::External(cmd_data.command.clone())
    };

    child::child_exec(child_sock, vars, cmd, &cmd_data.args, &cmd_data.redirects)
}
