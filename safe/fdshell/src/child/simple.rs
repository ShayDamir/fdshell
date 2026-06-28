use crate::state::ShellState;
use builtins::error::BuiltinError;
use error_stack::Report;
use std::ffi::CStr;
use std::io::Write;
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
    use error_stack::ResultExt;
    let p = std::env::current_dir().change_context(BuiltinError::Io)?;
    let mut lock = std::io::stdout().lock();
    lock.write_all(format!("{}\n", p.display()).as_bytes())
        .change_context(BuiltinError::Io)?;
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
    use error_stack::ResultExt;
    let mut lock = std::io::stdout().lock();
    for (i, arg) in refs.iter().enumerate() {
        if i > 0 {
            lock.write_all(b" ").change_context(BuiltinError::Io)?;
        }
        lock.write_all(arg.to_bytes())
            .change_context(BuiltinError::Io)?;
    }
    lock.write_all(b"\n").change_context(BuiltinError::Io)?;
    Ok(0)
}
