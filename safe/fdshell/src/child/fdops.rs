//! `lseek`, `ftruncate`, `fsync`, `fallocate` — file ops on an existing `%var`
//! fd.
//!
//! The first argument is always a `%var` fd-variable name (`lseek %fd OFFSET
//! [WHENCE]`, `ftruncate %fd [LENGTH]`, `fsync %fd`, `fallocate %fd OFFSET
//! LEN`). `lseek` prints the new offset; `ftruncate` without LENGTH truncates
//! at the current offset; `fallocate` preallocates space.

// mutants: -C --lib -C --test -C fdops -- -E 'test(/^child::fdops::/) | binary(fdops)'

pub(super) mod args;
mod parse;

use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};

use super::Ctx;

pub(super) fn handle_lseek(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::lseek_parse(ctx.refs, ctx.args)?;
    let fd = ctx
        .state
        .fd_var(&cfg.var)
        .ok_or(BuiltinError::FdVarNotFound)?;
    let pos = fd
        .fd
        .lseek(cfg.offset, cfg.whence)
        .change_context(BuiltinError::Syscall)?;
    let line = sys::format!("{pos}\n").change_context(BuiltinError::Io)?;
    sys::OUT.write_str(&line).change_context(BuiltinError::Io)?;
    Ok(0)
}

pub(super) fn handle_ftruncate(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::ftruncate_parse(ctx.refs, ctx.args)?;
    let fd = ctx
        .state
        .fd_var(&cfg.var)
        .ok_or(BuiltinError::FdVarNotFound)?;
    let length = match cfg.length {
        Some(n) => n,
        None => fd
            .fd
            .lseek(0, sys::fcntl::SEEK_CUR)
            .change_context(BuiltinError::Syscall)?,
    };
    fd.fd
        .ftruncate(length)
        .change_context(BuiltinError::Syscall)?;
    Ok(0)
}

pub(super) fn handle_fsync(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::fsync_parse(ctx.refs, ctx.args)?;
    let fd = ctx
        .state
        .fd_var(&cfg.var)
        .ok_or(BuiltinError::FdVarNotFound)?;
    fd.fd.fsync().change_context(BuiltinError::Syscall)?;
    Ok(0)
}

pub(super) fn handle_fallocate(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::fallocate_parse(ctx.refs, ctx.args)?;
    let fd = ctx
        .state
        .fd_var(&cfg.var)
        .ok_or(BuiltinError::FdVarNotFound)?;
    fd.fd
        .fallocate(0, cfg.offset, cfg.len)
        .change_context(BuiltinError::Syscall)?;
    Ok(0)
}

#[cfg(test)]
mod tests;
