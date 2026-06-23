#![forbid(unsafe_code)]
pub(crate) mod builtin;
mod delegated;
mod dispatch;
mod exec;
pub(crate) mod fdpass;
mod run;
mod simple;
use crate::parse::CommandLine;
use sys::ShortCStr;

pub use run::child_main;

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
