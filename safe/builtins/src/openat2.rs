use error_stack::{Report, ResultExt, bail};
use sys::{AtFd, ImportedFd};

use crate::error::BuiltinError;

pub mod parse;

pub fn openat2_exec(
    cfg: &parse::Openat2Config,
    sock: &sys::LocalFd,
) -> Result<(), Report<BuiltinError>> {
    let dirfd = cfg.dirfd.as_ref().map_or(AtFd::cwd(), ImportedFd::at);
    let how = sys::openat2::OpenHow {
        flags: cfg.how.flags,
        mode: cfg.how.mode,
        resolve: cfg.how.resolve,
    };
    let fd = sys::openat2::openat2(dirfd, cfg.path, &how).change_context(BuiltinError::Syscall)?;
    if let Some(ref_fd) = &cfg.same_as {
        verify_same_as(&fd, ref_fd)?;
    }
    sock.send_fd(&fd, c"openat2")
        .change_context(BuiltinError::SendFdFailed)?;
    Ok(())
}

/// `--same-as`: the opened fd must be the same `(dev, ino)` as the reference.
/// The opened fd pins the inode, so this checks what the open actually
/// resolved to. On mismatch `fd` is dropped (closed) by the caller before
/// `send_fd`, so the capture is never committed.
fn verify_same_as(fd: &sys::LocalFd, ref_fd: &ImportedFd) -> Result<(), Report<BuiltinError>> {
    let opened = fd.fstat().change_context(BuiltinError::Syscall)?;
    let reference = ref_fd.fstat().change_context(BuiltinError::Syscall)?;
    let same = opened.dev == reference.dev && opened.ino == reference.ino;
    if !same {
        bail!(BuiltinError::SameAsMismatch);
    }
    Ok(())
}
