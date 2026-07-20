use crate::state::ShellState;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(super) fn handle_exec_fd(
    _: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    let raw0 = args
        .first()
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    let varname = raw0
        .strip_prefix(b"%")
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    let fd = state
        .fds
        .get(&varname)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("var"))?;
    let args_slice = refs
        .get(1..)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    match crate::exec::exec_fd(
        fd,
        args_slice,
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
    let raw0 = args
        .first()
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    let varname = raw0
        .strip_prefix(b"%")
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    let dirfd = state
        .fds
        .get(&varname)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("var"))?;
    let pathname = args
        .get(1)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    let pathname = pathname.export();
    // execveat rejects CLOEXEC dirfds for relative paths; use export().
    let non_cloexec = dirfd
        .export()
        .change_context(builtins::error::BuiltinError::Syscall)?;
    let args_slice = refs
        .get(2..)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    // Same exec-without-fork semantics as exec_fd — always Ok(code).
    match crate::exec::exec_at(
        non_cloexec.at(),
        &pathname,
        args_slice,
        &state.environ,
        &state.exports,
        &state.env_filter,
        state.shell_sock.as_ref(),
    ) {
        Ok(()) => Ok(0),
        Err(report) => Ok(report.current_context().exit_code()),
    }
}
