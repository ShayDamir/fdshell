//! `lseek`, `ftruncate`, `fsync` — file ops on an existing `%var` fd.
//!
//! The first argument is always a `%var` fd-variable name (`lseek %fd OFFSET
//! [WHENCE]`, `ftruncate %fd [LENGTH]`, `fsync %fd`). `lseek` prints the new
//! offset; `ftruncate` without LENGTH truncates at the current offset.

mod args;
mod parse;

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::{LocalFd, ShortCStr};

pub(super) fn handle_lseek(
    _: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::lseek_parse(refs, args)?;
    let fd = resolve(&cfg.var, state)?;
    let pos = sys::rw::lseek(fd, cfg.offset, cfg.whence).change_context(BuiltinError::Syscall)?;
    let line = sys::format!("{pos}\n").change_context(BuiltinError::Io)?;
    sys::OUT.write_str(&line).change_context(BuiltinError::Io)?;
    Ok(0)
}

pub(super) fn handle_ftruncate(
    _: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::ftruncate_parse(refs, args)?;
    let fd = resolve(&cfg.var, state)?;
    let length = match cfg.length {
        Some(n) => n,
        None => {
            sys::rw::lseek(fd, 0, sys::fcntl::SEEK_CUR).change_context(BuiltinError::Syscall)?
        }
    };
    sys::ftruncate::ftruncate(fd, length).change_context(BuiltinError::Syscall)?;
    Ok(0)
}

pub(super) fn handle_fsync(
    _: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::fsync_parse(refs, args)?;
    let fd = resolve(&cfg.var, state)?;
    sys::fsync::fsync(fd).change_context(BuiltinError::Syscall)?;
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
