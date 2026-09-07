//! `flock %var [--shared] [--wait] [--nowait] [--unlock]` — advisory lock on
//! an existing `%var` fd.
//!
//! The lock is taken on the fd var's open file description, so it outlives
//! this builtin's child and is held by the shell until `--unlock` or the var
//! is closed. Coordinates processes by handle, never by path. Defaults to an
//! exclusive blocking lock.

mod parse;

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::{LocalFd, ShortCStr};

pub(super) fn handle_flock(
    _: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::flock_parse(refs, args)?;
    let fd = resolve(&cfg.var, state)?;
    sys::fileops::flock(fd, cfg.operation).change_context(BuiltinError::Syscall)?;
    Ok(0)
}

fn resolve<'a>(
    var: &ShortCStr,
    state: &'a ShellState,
) -> Result<&'a LocalFd, Report<BuiltinError>> {
    let found = state.fds.get(var).ok_or(BuiltinError::FdVarNotFound)?;
    Ok(&found.fd)
}

#[cfg(test)]
mod tests;
