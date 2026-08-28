use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, bail};
use sys::ShortCStr;

use super::delegated;
use super::exec_fd;
use super::explain;
use super::fdexplain;
use super::fdops;
use super::printf;
use super::resolve;
use super::simple;
use super::test;
use super::type_cmd;

type Handler =
    fn(ShortCStr, &[&CStr], &[ShortCStr], &ShellState) -> Result<i32, Report<BuiltinError>>;

pub(crate) const DISPATCH: &[(&[u8], Handler)] = &[
    (b"true", simple::handle_true),
    (b"false", simple::handle_false),
    (b"help", simple::handle_help),
    (b"pwd", simple::handle_pwd),
    (b"fchmod", delegated::handle_fchmod),
    (b"echo", simple::handle_echo),
    (b"explain", explain::handle_explain),
    (b"fdexplain", fdexplain::handle_fdexplain),
    (b"pipe", delegated::handle_pipe),
    (b"mkdirat", delegated::handle_mkdirat),
    (b"openat2", delegated::handle_openat2),
    (b"printf", printf::handle_printf),
    (b"renameat2", delegated::handle_renameat2),
    (b"timerfd", delegated::handle_timerfd),
    (b"eventfd", delegated::handle_eventfd),
    (b"fsync", fdops::handle_fsync),
    (b"ftruncate", fdops::handle_ftruncate),
    (b"lseek", fdops::handle_lseek),
    (b"exec_fd", exec_fd::handle_exec_fd),
    (b"exec_at", exec_fd::handle_exec_at),
    (b"resolve", resolve::handle_resolve),
    (b"test", test::handle_test),
    (b"[", test::handle_test),
    (b"type", type_cmd::handle_type),
];

pub(crate) fn is_dispatched(name: &ShortCStr) -> bool {
    DISPATCH.iter().any(|(known, _)| name.eq_bytes(known))
}

/// With `builtin_first` on, a bare name in the builtin table resolves as a
/// builtin without the `builtin` keyword; names containing `/` always reach
/// the external.
pub fn builtin_first(name: &ShortCStr, state: &ShellState) -> bool {
    state.options & crate::options::BUILTIN_FIRST != 0
        && !name.contains(b'/')
        && is_dispatched(name)
}

pub fn dispatch_builtin(
    name: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    for (known, handler) in DISPATCH {
        if name.eq_bytes(known) {
            return handler(name, refs, args, state);
        }
    }

    match crate::child::fdpass::dispatch(name.as_bytes().unwrap_or(&[]), args, state) {
        Some(Ok(v)) => Ok(v),
        Some(Err(report)) => Ok(match report.current_context() {
            crate::error::fdpass::FdPassError::SendFailed
            | crate::error::fdpass::FdPassError::Cloexec => sys::errno::EIO,
            crate::error::fdpass::FdPassError::NotFound
            | crate::error::fdpass::FdPassError::InvalidName
            | crate::error::fdpass::FdPassError::MissingArg => sys::errno::EINVAL,
        }),
        None => bail!(BuiltinError::Unknown),
    }
}
