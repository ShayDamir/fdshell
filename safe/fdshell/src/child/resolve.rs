use crate::state::ShellState;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
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
    let name_cstr = refs
        .first()
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    let mut name_short = ShortCStr::new();
    name_short
        .push_slice(name_cstr.to_bytes())
        .change_context(builtins::error::BuiltinError::Never)?;
    let fd = crate::exec::resolve_path(&name_short)
        .change_context(builtins::error::BuiltinError::InvalidArgument("path"))?;
    sys::shellfd::send_fd(sock, &fd, c"resolve")
        .change_context(builtins::error::BuiltinError::SendFdFailed)?;
    Ok(0)
}
