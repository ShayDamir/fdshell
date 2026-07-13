use error_stack::{Report, ResultExt};
use sys::fcntl::O_CLOEXEC;
use sys::{AtFd, ImportedFd};

use crate::error::BuiltinError;

pub mod parse;

pub fn openat2_exec(
    cfg: &parse::Openat2Config,
    sock: &sys::LocalFd,
) -> Result<(), Report<BuiltinError>> {
    let dirfd = cfg.dirfd.as_ref().map_or(AtFd::cwd(), ImportedFd::at);
    let how = sys::openat2::OpenHow {
        flags: cfg.how.flags | (O_CLOEXEC as u64),
        mode: cfg.how.mode,
        resolve: cfg.how.resolve,
    };
    let fd = sys::openat2::openat2(dirfd, cfg.path, &how).change_context(BuiltinError::Syscall)?;
    sys::shellfd::send_fd(sock, &fd, c"openat2").change_context(BuiltinError::SendFdFailed)?;
    Ok(())
}
