use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::{ExportedCStr, ShortCStr};

use super::Ctx;

fn lookup_var(args: &[ShortCStr]) -> Result<ShortCStr, Report<builtins::error::BuiltinError>> {
    Ok(args
        .first()
        .and_then(|a| a.strip_prefix(b"%"))
        .ok_or(builtins::error::BuiltinError::InvalidArgument("var"))?)
}

pub(super) fn handle_exec_fd(ctx: &Ctx) -> Result<i32, Report<builtins::error::BuiltinError>> {
    let _sealed: Vec<ExportedCStr> = ctx.args.iter().map(|a| a.export()).collect();
    let words: Vec<&CStr> = _sealed.iter().map(|s| s.as_ref()).collect();
    builtins::execfd::parse::execfd_parse(&words)?;
    let varname = lookup_var(ctx.args)?;
    let var = ctx
        .state
        .fds
        .get(&varname)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("var"))?;
    let argv = ctx
        .refs
        .get(1..)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    // Same exec-without-fork semantics as external commands — always Ok(code).
    match crate::exec::exec_fd(
        &var.fd,
        argv,
        &ctx.state.environ,
        &ctx.state.exports,
        &ctx.state.env_filter,
        ctx.state.shell_sock.as_ref(),
    ) {
        Ok(()) => Ok(0),
        Err(report) => Ok(report.current_context().exit_code()),
    }
}

pub(super) fn handle_exec_at(ctx: &Ctx) -> Result<i32, Report<builtins::error::BuiltinError>> {
    let _sealed: Vec<ExportedCStr> = ctx.args.iter().map(|a| a.export()).collect();
    let words: Vec<&CStr> = _sealed.iter().map(|s| s.as_ref()).collect();
    let cfg = builtins::execat::parse::execat_parse(&words)?;
    let varname = lookup_var(ctx.args)?;
    let dirfd = ctx
        .state
        .fds
        .get(&varname)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("var"))?;
    // execveat rejects CLOEXEC dirfds for relative paths; use export().
    let non_cloexec = dirfd
        .fd
        .export()
        .change_context(builtins::error::BuiltinError::Syscall)?;
    let argv = ctx
        .refs
        .get(2..)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    match crate::exec::exec_at(
        non_cloexec.at(),
        cfg.pathname,
        argv,
        &ctx.state.environ,
        &ctx.state.exports,
        &ctx.state.env_filter,
        ctx.state.shell_sock.as_ref(),
    ) {
        Ok(()) => Ok(0),
        Err(report) => Ok(report.current_context().exit_code()),
    }
}
