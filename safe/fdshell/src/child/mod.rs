#![forbid(unsafe_code)]
mod builtin;
mod run;
use crate::redirect::Redirect;
use crate::vars::FdVars;
use sys::ShortCStr;

pub enum Command {
    Builtin(ShortCStr),
    External(ShortCStr),
}

pub fn child_exec(
    child_sock: Option<sys::Fd>,
    vars: &FdVars,
    cmd: Command,
    args: &[ShortCStr],
    redirects: &[Redirect],
) -> ! {
    match run::child_main(child_sock, vars, cmd, args, redirects) {
        Ok(()) => std::process::exit(0),
        Err(e) => std::process::exit(e),
    }
}
