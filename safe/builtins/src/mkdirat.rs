//! `mkdirat` builtin: create a directory inside a directory and open it.
//!
//! `mkdirat [--dirfd N] [--mode MODE] [--resolve FLAGS] path` creates the
//! directory via `mkdirat`, reopens it read-only, and exports the handle to
//! the parent shell tagged `dirfd`. An omitted `--mode` is `0777`, which the
//! kernel masks with the umask (`umask 022` → `0755`); see [`crate::mode`].

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
