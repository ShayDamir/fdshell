use crate::state::ShellState;
use core::ffi::CStr;
use error_stack::Report;
use sys::ShortCStr;

pub(super) fn handle_fchmod(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    let cfg = builtins::fchmod::parse::fchmod_parse(refs)?;
    builtins::fchmod::fchmod_exec(&cfg).map(|()| 0)
}

pub(super) fn handle_pipe(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    let sock = state
        .shell_sock
        .as_ref()
        .ok_or(builtins::error::BuiltinError::SendFdFailed)?;
    let cfg = builtins::pipe::parse::pipe_parse(refs)?;
    builtins::pipe::pipe_exec(cfg.flags, sock).map(|()| 0)
}

pub(super) fn handle_mkdirat(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    let sock = state
        .shell_sock
        .as_ref()
        .ok_or(builtins::error::BuiltinError::SendFdFailed)?;
    let cfg = builtins::mkdirat::parse::mkdirat_parse(refs)?;
    builtins::mkdirat::mkdirat_exec(&cfg, sock).map(|()| 0)
}

pub(super) fn handle_openat2(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    let sock = state
        .shell_sock
        .as_ref()
        .ok_or(builtins::error::BuiltinError::SendFdFailed)?;
    let cfg = builtins::openat2::parse::openat2_parse(refs)?;
    builtins::openat2::openat2_exec(&cfg, sock).map(|()| 0)
}

pub(super) fn handle_renameat2(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    let cfg = builtins::renameat2::parse::renameat2_parse(refs)?;
    builtins::renameat2::renameat2_exec(&cfg).map(|()| 0)
}
