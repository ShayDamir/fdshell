use error_stack::{Report, ResultExt};
use sys::{AtFd, ImportedFd};

use crate::error::BuiltinError;

pub mod parse;

pub fn mkdirat_exec(
    cfg: &parse::MkdiratConfig,
    sock: &sys::LocalFd,
) -> Result<(), Report<BuiltinError>> {
    let dirfd = cfg.dirfd.as_ref().map_or(AtFd::cwd(), ImportedFd::at);
    sys::fileat::mkdirat(dirfd, cfg.path, cfg.mode & 0o777)
        .change_context(BuiltinError::Syscall)?;
    let how = sys::openat2::OpenHow {
        flags: 0,
        mode: 0,
        resolve: cfg.resolve,
    };
    let fd = sys::openat2::openat2(dirfd, cfg.path, &how).change_context(BuiltinError::Syscall)?;
    sock.send_fd(&fd, c"dirfd")
        .change_context(BuiltinError::SendFdFailed)?;
    Ok(())
}
