use error_stack::{Report, ResultExt};

use crate::error::BuiltinError;

pub mod parse;

/// Create an eventfd and send it to the shell.
pub fn eventfd_exec(
    cfg: &parse::EventfdConfig,
    sock: &sys::LocalFd,
) -> Result<(), Report<BuiltinError>> {
    let fd = sys::eventfd::eventfd(cfg.init, cfg.flags).change_context(BuiltinError::Syscall)?;
    sys::shellfd::send_fd(sock, &fd, c"eventfd").change_context(BuiltinError::SendFdFailed)?;
    Ok(())
}
