use crate::state::ShellState;
use builtins::error::BuiltinError;
use std::ffi::CStr;
use sys::ShortCStr;

pub(super) fn handle_true(
    _: ShortCStr,
    _: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, BuiltinError> {
    Ok(0)
}

pub(super) fn handle_false(
    _: ShortCStr,
    _: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, BuiltinError> {
    Ok(1)
}

pub(super) fn handle_pwd(
    _: ShortCStr,
    _: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, BuiltinError> {
    let p = std::env::current_dir().map_err(|_| BuiltinError::InvalidArgument)?;
    println!("{}", p.display());
    Ok(0)
}

pub(super) fn handle_help(
    _: ShortCStr,
    _: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, BuiltinError> {
    crate::child::help::print_help()
}

pub(super) fn handle_echo(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    _: &ShellState,
) -> Result<i32, BuiltinError> {
    use std::io::Write;
    let mut lock = std::io::stdout().lock();
    for (i, arg) in refs.iter().enumerate() {
        if i > 0 {
            let _ = lock.write_all(b" ");
        }
        let _ = lock.write_all(arg.to_bytes());
    }
    let _ = lock.write_all(b"\n");
    Ok(0)
}
