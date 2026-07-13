use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use std::ffi::CStr;
use sys::ShortCStr;

pub(super) fn handle_resolve(
    _: ShortCStr,
    refs: &[&CStr],
    _: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<builtins::error::BuiltinError>> {
    let sock = state
        .shell_sock
        .as_ref()
        .ok_or(builtins::error::BuiltinError::SendFdFailed)?;
    let name = refs
        .first()
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    let fd = crate::exec::resolve_path(name)
        .change_context(builtins::error::BuiltinError::InvalidArgument("path"))?;
    sys::shellfd::send_fd(sock, &fd, c"resolve")
        .change_context(builtins::error::BuiltinError::SendFdFailed)?;
    Ok(0)
}
