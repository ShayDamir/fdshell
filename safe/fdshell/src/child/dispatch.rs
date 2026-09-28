mod table;

use crate::state::ShellState;
use builtins::error::BuiltinError;
use core::ffi::CStr;
use error_stack::{Report, bail};
use sys::ShortCStr;

use super::Ctx;

pub(crate) use table::DISPATCH;

pub(crate) fn is_dispatched(name: &ShortCStr) -> bool {
    DISPATCH.iter().any(|(known, _)| name.eq_bytes(known))
}

/// With `builtin_first` on, a bare name in the builtin table resolves as a
/// builtin without the `builtin` keyword; names containing `/` always reach
/// the external.
pub fn builtin_first(name: &ShortCStr, state: &ShellState) -> bool {
    state.options & crate::options::BUILTIN_FIRST != 0
        && !name.contains(b'/')
        && is_dispatched(name)
}

pub fn dispatch_builtin(
    name: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<BuiltinError>> {
    let ctx = Ctx::new(name, refs, args, state);
    for (known, handler) in DISPATCH {
        if ctx.name.eq_bytes(known) {
            return handler(&ctx);
        }
    }

    match crate::child::fdpass::dispatch(ctx.name.as_bytes().unwrap_or(&[]), ctx.args, ctx.state) {
        Some(Ok(v)) => Ok(v),
        Some(Err(report)) => Ok(match report.current_context() {
            crate::error::fdpass::FdPassError::SendFailed
            | crate::error::fdpass::FdPassError::Cloexec => sys::errno::EIO,
            crate::error::fdpass::FdPassError::NotFound
            | crate::error::fdpass::FdPassError::InvalidName
            | crate::error::fdpass::FdPassError::MissingArg => sys::errno::EINVAL,
        }),
        None => bail!(BuiltinError::Unknown),
    }
}
