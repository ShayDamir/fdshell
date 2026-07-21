use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub(super) fn handle_true(
    _: ShortCStr,
    _: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    Ok(0)
}

pub(super) fn handle_false(
    _: ShortCStr,
    _: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    Ok(1)
}

pub(super) fn handle_pwd(
    _: ShortCStr,
    _: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let cwd = sys::env::getcwd().change_context(BuiltinError::Io)?;
    sys::OUT.write_all(&cwd).change_context(BuiltinError::Io)?;
    sys::OUT.write_all(b"\n").change_context(BuiltinError::Io)?;
    Ok(0)
}

pub(super) fn handle_help(
    _: ShortCStr,
    _: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    crate::child::help::print_help()
}

pub(super) fn handle_echo(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    for (i, arg) in refs.iter().enumerate() {
        if i > 0 {
            sys::OUT.write_all(b" ").change_context(BuiltinError::Io)?;
        }
        sys::OUT
            .write_all(arg.to_bytes())
            .change_context(BuiltinError::Io)?;
    }
    sys::OUT.write_all(b"\n").change_context(BuiltinError::Io)?;
    Ok(0)
}
