mod accept;
mod bind;
mod connect;
mod copy_file_range;
mod delegated;
pub(crate) mod dispatch;
mod error;
pub(crate) mod exec_fd;
mod explain;
pub(crate) mod external;
mod fdexplain;
mod fdops;
pub(crate) mod fdpass;
mod flags;
mod flock;
mod help;
mod listen;
mod ls;
mod netaddr;
mod printf;
mod readlink;
mod resolve;
mod run;
mod simple;
mod statx;
mod test;
mod type_cmd;
mod verity;
use crate::parse::{BuiltinPrefix, CommandLine};
use crate::state::ShellState;
use alloc::vec::Vec;
use core::ffi::CStr;
use sys::ShortCStr;

pub(crate) use error::handle_builtin_error;
pub use run::child_main;

pub struct Command {
    pub prefix: BuiltinPrefix,
    pub name: ShortCStr,
    /// Per-byte quote mask for the command word (word 0); its unquoted
    /// pattern bytes glob the command name (bash field model).
    pub command_mask: Vec<bool>,
}

/// Everything a builtin handler may need: the command name, the substituted
/// argument references, the raw args, and the shell state.
pub struct Ctx<'a> {
    pub(crate) name: ShortCStr,
    pub(crate) refs: &'a [&'a CStr],
    pub(crate) args: &'a [ShortCStr],
    pub(crate) state: &'a ShellState,
}

impl<'a> Ctx<'a> {
    fn new(
        name: ShortCStr,
        refs: &'a [&'a CStr],
        args: &'a [ShortCStr],
        state: &'a ShellState,
    ) -> Self {
        Self {
            name,
            refs,
            args,
            state,
        }
    }
}

impl From<&CommandLine> for Command {
    fn from(cmdline: &CommandLine) -> Self {
        Command {
            prefix: cmdline.prefix,
            name: cmdline.command.clone(),
            command_mask: cmdline.command_mask.clone(),
        }
    }
}
