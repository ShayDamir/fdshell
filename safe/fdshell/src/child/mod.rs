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

pub struct Command {
    pub builtin: bool,
    pub name: ShortCStr,
}

impl From<&CommandLine> for Command {
    fn from(cmdline: &CommandLine) -> Self {
        Command {
            builtin: cmdline.builtin,
            name: cmdline.command.clone(),
        }
    }
}
