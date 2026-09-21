//! `lseek`, `ftruncate`, `fsync`, `fallocate` — file ops on an existing `%var`
//! fd.
//!
//! The first argument is always a `%var` fd-variable name (`lseek %fd OFFSET
//! [WHENCE]`, `ftruncate %fd [LENGTH]`, `fsync %fd`, `fallocate %fd OFFSET
//! LEN`). `lseek` prints the new offset; `ftruncate` without LENGTH truncates
//! at the current offset; `fallocate` preallocates space.

pub(super) mod args;
mod parse;

use crate::state::ShellState;
use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};
use sys::{LocalFd, ShortCStr};

use super::Ctx;

pub(super) fn handle_lseek(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::lseek_parse(ctx.refs, ctx.args)?;
    let fd = resolve(&cfg.var, ctx.state)?;
    let pos = sys::rw::lseek(fd, cfg.offset, cfg.whence).change_context(BuiltinError::Syscall)?;
    let line = sys::format!("{pos}\n").change_context(BuiltinError::Io)?;
    sys::OUT.write_str(&line).change_context(BuiltinError::Io)?;
    Ok(0)
}

pub(super) fn handle_ftruncate(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::ftruncate_parse(ctx.refs, ctx.args)?;
    let fd = resolve(&cfg.var, ctx.state)?;
    let length = match cfg.length {
        Some(n) => n,
        None => {
            sys::rw::lseek(fd, 0, sys::fcntl::SEEK_CUR).change_context(BuiltinError::Syscall)?
        }
    };
    sys::fileops::ftruncate(fd, length).change_context(BuiltinError::Syscall)?;
    Ok(0)
}

pub(super) fn handle_fsync(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::fsync_parse(ctx.refs, ctx.args)?;
    let fd = resolve(&cfg.var, ctx.state)?;
    sys::fileops::fsync(fd).change_context(BuiltinError::Syscall)?;
    Ok(0)
}

pub(super) fn handle_fallocate(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::fallocate_parse(ctx.refs, ctx.args)?;
    let fd = resolve(&cfg.var, ctx.state)?;
    sys::fileops::fallocate(fd, 0, cfg.offset, cfg.len).change_context(BuiltinError::Syscall)?;
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
