use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use std::ffi::CStr;
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
    let exports: Vec<(sys::ShortCStr, Vec<u8>)> = state
        .exports
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let args_slice = refs
        .get(1..)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    // exec-without-fork: PID stays the same, so return child exit code
    // regardless of outcome. Err arm only catches parse/syscall errors.
    match crate::exec::exec_fd(fd, args_slice, &exports) {
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
    let pathname = sys::RefCStr::from(pathname.clone());
    // execveat rejects CLOEXEC dirfds for relative paths; use export().
    let non_cloexec = dirfd
        .export()
        .change_context(builtins::error::BuiltinError::Syscall)?;
    let exports: Vec<(sys::ShortCStr, Vec<u8>)> = state
        .exports
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let args_slice = refs
        .get(2..)
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    // Same exec-without-fork semantics as exec_fd — always Ok(code).
    match crate::exec::exec_at(non_cloexec.at(), &pathname, args_slice, &exports) {
        Ok(()) => Ok(0),
        Err(report) => Ok(report.current_context().exit_code()),
    }
}

pub(super) fn handle_resolve(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    use error_stack::ResultExt;
    let name = refs
        .first()
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    let fd = crate::exec::resolve_path(name)
        .change_context(builtins::error::BuiltinError::InvalidArgument("path"))?;
    sys::shellfd::send_fd(&fd, c"resolve").ok();
    Ok(0)
}
