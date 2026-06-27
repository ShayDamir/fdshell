use crate::state::ShellState;
use builtins::error::BuiltinError;
use std::ffi::CStr;
use sys::ShortCStr;

use super::delegated;
use super::exec;
use super::simple;

type Handler = fn(ShortCStr, &[&CStr], &[ShortCStr], &ShellState) -> Result<i32, BuiltinError>;

const DISPATCH: &[(&[u8], Handler)] = &[
    (b"true", simple::handle_true),
    (b"false", simple::handle_false),
    (b"help", simple::handle_help),
    (b"pwd", simple::handle_pwd),
    (b"fchmod", delegated::handle_fchmod),
    (b"echo", simple::handle_echo),
    (b"pipe", delegated::handle_pipe),
    (b"mkdirat", delegated::handle_mkdirat),
    (b"openat2", delegated::handle_openat2),
    (b"renameat2", delegated::handle_renameat2),
    (b"exec_fd", exec::handle_exec_fd),
    (b"exec_at", exec::handle_exec_at),
    (b"resolve", exec::handle_resolve),
];

pub fn dispatch_builtin(
    name: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, BuiltinError> {
    let _ = name.as_bytes().map_err(|_| BuiltinError::InvalidArgument)?;

    for (known, handler) in DISPATCH {
        if name.eq_bytes(known) {
            return handler(name, refs, args, state);
        }
    }

    match crate::child::fdpass::dispatch(name.as_bytes().unwrap_or(&[]), args, state) {
        Some(Ok(v)) => Ok(v),
        Some(Err(report)) => Ok(match report.current_context() {
            crate::error::fdpass::FdPassError::SendFailed => sys::errno::EIO,
            crate::error::fdpass::FdPassError::NotFound
            | crate::error::fdpass::FdPassError::InvalidName
            | crate::error::fdpass::FdPassError::MissingArg => sys::errno::EINVAL,
        }),
        None => Err(BuiltinError::Unknown),
    }
}
