mod delegated;
pub(crate) mod dispatch;
mod error;
pub(crate) mod exec_fd;
mod explain;
pub(crate) mod external;
mod fdexplain;
pub(crate) mod fdpass;
mod help;
mod printf;
mod resolve;
mod run;
mod simple;
mod test;
use crate::parse::CommandLine;
use sys::ShortCStr;

pub(crate) use error::handle_builtin_error;
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
