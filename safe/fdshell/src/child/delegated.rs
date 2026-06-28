use crate::state::ShellState;
use error_stack::Report;
use std::ffi::CStr;
use sys::ShortCStr;

pub(super) fn handle_fchmod(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    Ok(builtins::fchmod::parse::fchmod_parse(refs)
        .and_then(|cfg| builtins::fchmod::fchmod_exec(&cfg))
        .map(|()| 0)?)
}

pub(super) fn handle_pipe(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    Ok(builtins::pipe::parse::pipe_parse(refs)
        .and_then(|cfg| builtins::pipe::pipe_exec(cfg.flags))
        .map(|()| 0)?)
}

pub(super) fn handle_mkdirat(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    Ok(builtins::mkdirat::parse::mkdirat_parse(refs)
        .and_then(|cfg| builtins::mkdirat::mkdirat_exec(&cfg))
        .map(|()| 0)?)
}

pub(super) fn handle_openat2(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    Ok(builtins::openat2::parse::openat2_parse(refs)
        .and_then(|cfg| builtins::openat2::openat2_exec(&cfg))
        .map(|()| 0)?)
}

pub(super) fn handle_renameat2(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    Ok(builtins::renameat2::parse::renameat2_parse(refs)
        .and_then(|cfg| builtins::renameat2::renameat2_exec(&cfg))
        .map(|()| 0)?)
}
