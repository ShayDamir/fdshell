//! `mkfifoat` builtin: create a FIFO inside a directory and open it read-write.
//!
//! `mkfifoat [--dirfd N] [--mode MODE] [--resolve FLAGS] path` creates a FIFO
//! via `mkfifoat`, then reopens it with `O_RDWR` — a non-blocking self-pipe:
//! `open` on a FIFO with `O_RDWR` succeeds with no peer end, and the handle
//! reads back what is written to it. The handle is sent to the capture
//! socket tagged `fifo`, giving a temp-file-less message-passing channel.

pub mod parse;

use error_stack::{Report, ResultExt};
use sys::{AtFd, ImportedFd};

use crate::error::BuiltinError;

/// Create the FIFO, reopen it read-write, and export it to the parent shell.
pub fn mkfifoat_exec(
    cfg: &parse::MkfifoatConfig,
    sock: &sys::LocalFd,
) -> Result<(), Report<BuiltinError>> {
    let dirfd = cfg.dirfd.as_ref().map_or(AtFd::cwd(), ImportedFd::at);
    sys::fileat::mkfifoat(dirfd, cfg.path, cfg.mode & 0o777)
        .change_context(BuiltinError::Syscall)?;
    let how = sys::openat2::OpenHow {
        flags: sys::fcntl::O_RDWR as u64,
        mode: 0,
        resolve: cfg.resolve,
    };
    let fd = sys::openat2::openat2(dirfd, cfg.path, &how).change_context(BuiltinError::Syscall)?;
    sock.send_fd(&fd, c"fifo")
        .change_context(BuiltinError::SendFdFailed)?;
    Ok(())
}
