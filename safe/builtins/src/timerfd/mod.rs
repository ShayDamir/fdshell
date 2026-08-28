use error_stack::{Report, ResultExt};

use crate::error::BuiltinError;

pub mod parse;

/// Create a `CLOCK_MONOTONIC` timerfd, arm it, and send it to the shell.
pub fn timerfd_exec(
    cfg: &parse::TimerfdConfig,
    sock: &sys::LocalFd,
) -> Result<(), Report<BuiltinError>> {
    let fd = sys::timerfd::timerfd_create(cfg.flags).change_context(BuiltinError::Syscall)?;
    sys::timerfd::timerfd_settime(
        &fd,
        (cfg.value_sec, cfg.value_nsec),
        (cfg.interval_sec, cfg.interval_nsec),
    )
    .change_context(BuiltinError::Syscall)?;
    sys::shellfd::send_fd(sock, &fd, c"timerfd").change_context(BuiltinError::SendFdFailed)?;
    Ok(())
}
