#![forbid(unsafe_code)]
pub(crate) mod builtin;
pub(crate) mod fdpass;
mod run;
use crate::parse::CommandLine;
use crate::redirect::Redirect;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub enum Command {
    Builtin(ShortCStr),
    External(ShortCStr),
}

impl From<&CommandLine> for Command {
    fn from(cmdline: &CommandLine) -> Self {
        if cmdline.builtin {
            Command::Builtin(cmdline.command.clone())
        } else {
            Command::External(cmdline.command.clone())
        }
    }
}

pub fn child_exec(
    child_sock: Option<sys::LocalFd>,
    cell: &ForkCell<ShellState>,
    cmd: Command,
    args: &[ShortCStr],
    redirects: &[Redirect],
) -> ! {
    match run::child_main(child_sock, cell, cmd, args, redirects) {
        Ok(()) => std::process::exit(0),
        Err(e) => std::process::exit(e),
    }
}
