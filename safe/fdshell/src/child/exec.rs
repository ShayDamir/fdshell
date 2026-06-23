use crate::state::ShellState;
use builtins::error::BuiltinError;
use std::ffi::CStr;
use sys::ShortCStr;

pub(super) fn handle_exec_fd(
    _: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, BuiltinError> {
    let raw0 = args.first().ok_or(BuiltinError::InvalidArgument)?;
    let varname = raw0
        .strip_prefix(b"%")
        .ok_or(BuiltinError::InvalidArgument)?;
    let fd = state
        .fds
        .get(&varname)
        .ok_or(BuiltinError::InvalidArgument)?;
    let exports: Vec<(sys::ShortCStr, Vec<u8>)> = state
        .exports
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let args_slice = refs.get(1..).ok_or(BuiltinError::InvalidArgument)?;
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
) -> Result<i32, BuiltinError> {
    let raw0 = args.first().ok_or(BuiltinError::InvalidArgument)?;
    let varname = raw0
        .strip_prefix(b"%")
        .ok_or(BuiltinError::InvalidArgument)?;
    let dirfd = state
        .fds
        .get(&varname)
        .ok_or(BuiltinError::InvalidArgument)?;
    let pathname = args.get(1).ok_or(BuiltinError::InvalidArgument)?;
    let pathname = sys::RefCStr::from(pathname.clone());
    // execveat rejects CLOEXEC dirfds for relative paths; use export().
    let non_cloexec = dirfd.export().map_err(BuiltinError::from)?;
    let exports: Vec<(sys::ShortCStr, Vec<u8>)> = state
        .exports
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let args_slice = refs.get(2..).ok_or(BuiltinError::InvalidArgument)?;
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
) -> Result<i32, BuiltinError> {
    let name = refs.first().ok_or(BuiltinError::InvalidArgument)?;
    let fd = crate::exec::resolve_path(name).map_err(|_| BuiltinError::InvalidArgument)?;
    sys::shellfd::send_fd(&fd, c"resolve").ok();
    Ok(0)
}
