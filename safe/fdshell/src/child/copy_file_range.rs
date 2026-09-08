//! `copy_file_range %in %out [COUNT]` — zero-copy copy of bytes between two
//! `%var` fds using `copy_file_range(2)`. Prints the number of bytes copied.

use crate::state::ShellState;
use builtins::copy_file_range::{copy_file_range_exec, parse::copy_file_range_parse};
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::{LocalFd, ShortCStr};

pub(super) fn handle_copy_file_range(
    _: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let cfg = copy_file_range_parse(refs, args)?;
    let in_fd = resolve(&cfg.in_var, state)?;
    let out_fd = resolve(&cfg.out_var, state)?;
    let copied = copy_file_range_exec(&cfg, in_fd, out_fd)?;
    let line = sys::format!("{copied}\n").change_context(BuiltinError::Io)?;
    sys::OUT.write_str(&line).change_context(BuiltinError::Io)?;
    Ok(0)
}

fn resolve<'a>(
    var: &ShortCStr,
    state: &'a ShellState,
) -> Result<&'a LocalFd, Report<BuiltinError>> {
    let found = state.fds.get(var).ok_or(BuiltinError::FdVarNotFound)?;
    Ok(&found.fd)
}

#[cfg(test)]
mod tests;
