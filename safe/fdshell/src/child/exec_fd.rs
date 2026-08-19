use crate::state::ShellState;
use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::{ExportedCStr, ShortCStr};

fn lookup_var(args: &[ShortCStr]) -> Result<ShortCStr, Report<builtins::error::BuiltinError>> {
    Ok(args
        .first()
        .and_then(|a| a.strip_prefix(b"%"))
        .ok_or(builtins::error::BuiltinError::InvalidArgument("var"))?)
}

pub(super) fn handle_exec_fd(
    _: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    let _sealed: Vec<ExportedCStr> = args.iter().map(|a| a.export()).collect();
    let words: Vec<&CStr> = _sealed.iter().map(|s| s.as_ref()).collect();
    builtins::execfd::parse::execfd_parse(&words)?;
    let varname = lookup_var(args)?;
    let fd = state
        .fds
        .get(&varname)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("var"))?;
    let argv = refs
        .get(1..)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    // Same exec-without-fork semantics as external commands — always Ok(code).
    match crate::exec::exec_fd(
        fd,
        argv,
        &state.environ,
        &state.exports,
        &state.env_filter,
        state.shell_sock.as_ref(),
    ) {
        Ok(()) => Ok(0),
        Err(report) => Ok(report.current_context().exit_code()),
    }
}

pub(super) fn handle_exec_at(
    _: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    let _sealed: Vec<ExportedCStr> = args.iter().map(|a| a.export()).collect();
    let words: Vec<&CStr> = _sealed.iter().map(|s| s.as_ref()).collect();
    let cfg = builtins::execat::parse::execat_parse(&words)?;
    let varname = lookup_var(args)?;
    let dirfd = state
        .fds
        .get(&varname)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("var"))?;
    // execveat rejects CLOEXEC dirfds for relative paths; use export().
    let non_cloexec = dirfd
        .export()
        .change_context(builtins::error::BuiltinError::Syscall)?;
    let argv = refs
        .get(2..)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    match crate::exec::exec_at(
        non_cloexec.at(),
        cfg.pathname,
        argv,
        &state.environ,
        &state.exports,
        &state.env_filter,
        state.shell_sock.as_ref(),
    ) {
        Ok(()) => Ok(0),
        Err(report) => Ok(report.current_context().exit_code()),
    }
}
