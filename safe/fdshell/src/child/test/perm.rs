//! Permission tests of `test` (`-r -w -x`) via `access(2)`. A `%var` operand
//! is checked through its `/proc/self/fd/N` symlink, which `access` follows
//! to the real file.

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::{ExportedCStr, ShortCStr};

use super::stat::fd_var;

/// `-r -w -x`: true iff the operand's file allows the access mode.
pub(super) fn access_test(
    op: &[u8],
    arg: &CStr,
    orig: Option<&ShortCStr>,
    state: &ShellState,
) -> Result<bool, Report<BuiltinError>> {
    let mode = match op {
        b"-r" => sys::access::R_OK,
        b"-w" => sys::access::W_OK,
        _ => sys::access::X_OK,
    };
    let proc_path = fd_var(orig, state)
        .map(|fd| sys::format!("/proc/self/fd/{}", fd.as_raw()).change_context(BuiltinError::Never))
        .transpose()?
        .map(ExportedCStr::from);
    let target = match &proc_path {
        Some(path) => path.as_ref(),
        None => arg,
    };
    Ok(sys::access::access(target, mode).is_ok())
}
