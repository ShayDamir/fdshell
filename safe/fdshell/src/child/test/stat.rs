use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

/// Stat the operand: a `%var` original argument is resolved through the fd
/// table; anything else is a path. `None` means unset or nonexistent.
pub(super) fn stat_operand(
    arg: &CStr,
    orig: Option<&ShortCStr>,
    state: &ShellState,
) -> Result<Option<sys::stat::FileStat>, Report<BuiltinError>> {
    if let Some(var) = orig.and_then(|o| o.strip_prefix(b"%")) {
        return match state.fds.get(&var) {
            Some(v) => Ok(Some(
                sys::stat::fstat(&v.fd).change_context(BuiltinError::Syscall)?,
            )),
            None => Ok(None),
        };
    }
    // A stat failure (e.g. ENOENT) is false, matching bash.
    match sys::stat::stat(arg) {
        Ok(st) => Ok(Some(st)),
        Err(_) => Ok(None),
    }
}
