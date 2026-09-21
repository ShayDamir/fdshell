use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use super::Ctx;

pub(super) fn handle_resolve(ctx: &Ctx) -> Result<i32, Report<builtins::error::BuiltinError>> {
    let sock = ctx
        .state
        .shell_sock
        .as_ref()
        .ok_or(builtins::error::BuiltinError::SendFdFailed)?;
    let name_cstr = ctx
        .refs
        .first()
        .ok_or(builtins::error::BuiltinError::InvalidArgument("arg"))?;
    let mut name_short = ShortCStr::new();
    name_short.push(*name_cstr);
    let fd = crate::exec::resolve_path(&name_short, &ctx.state.hash_table)
        .change_context(builtins::error::BuiltinError::InvalidArgument("path"))?;
    sys::shellfd::send_fd(sock, &fd, c"resolve")
        .change_context(builtins::error::BuiltinError::SendFdFailed)?;
    Ok(0)
}
