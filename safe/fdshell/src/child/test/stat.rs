use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::{LocalFd, ShortCStr};

/// Stat an operand. A `%var` original is an fd var (fstat it); anything else
/// is a path. `follow_symlink` picks `stat` (follow) vs `lstat` for paths.
/// `None` means the var is unset or the path does not exist.
pub(super) fn stat_operand(
    arg: &CStr,
    orig: Option<&ShortCStr>,
    state: &ShellState,
    follow_symlink: bool,
) -> Result<Option<sys::stat::FileStat>, Report<BuiltinError>> {
    if let Some(fd) = fd_var(orig, state) {
        return Ok(Some(
            sys::stat::fstat(fd).change_context(BuiltinError::Syscall)?,
        ));
    }
    let res = if follow_symlink {
        sys::stat::stat(arg)
    } else {
        sys::stat::lstat(arg)
    };
    match res {
        Ok(st) => Ok(Some(st)),
        Err(_) => Ok(None),
    }
}

/// The `LocalFd` backing a `%name` operand, if it names a set fd variable.
pub(super) fn fd_var<'a>(orig: Option<&ShortCStr>, state: &'a ShellState) -> Option<&'a LocalFd> {
    orig.and_then(|o| o.strip_prefix(b"%"))
        .and_then(|name| state.fds.get(&name))
        .map(|var| &var.fd)
}
