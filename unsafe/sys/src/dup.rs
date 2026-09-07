use crate::{LocalFd, SyscallError, cvt};

/// Close a process fd by number.
///
/// The caller must own the fd (be about to drop it) or be deliberately
/// closing it, e.g. an `N>&-` redirection dropping the shell's own fd.
pub fn close(fd: i32) -> Result<(), SyscallError> {
    // SAFETY: caller guarantees exclusive ownership of `fd`; `close` on an
    // invalid or already-closed fd safely returns -1/EBADF, caught by `cvt`.
    cvt(unsafe { libc::close(fd) as isize }).map(|_| ())
}

/// Dup `raw` into a new `CLOEXEC` fd owned by the caller.
///
/// The returned `LocalFd` is a distinct fd-table entry from `raw`, so dropping it
/// never closes `raw`. Used to hand an ephemeral, independently-closed copy to a
/// forked arm child without aliasing the parent's descriptor.
pub fn dup_cloexec(raw: i32) -> Result<LocalFd, SyscallError> {
    // SAFETY: `F_DUPFD_CLOEXEC` on an invalid `raw` returns -1/EBADF, caught by
    // `cvt`; on success it returns the lowest free fd with CLOEXEC set, a fresh
    // fd-table entry distinct from `raw` (so `raw` is never closed).
    let ret = cvt(unsafe { libc::fcntl(raw, libc::F_DUPFD_CLOEXEC, 0) as isize })?;
    // SAFETY: `ret` is a valid new fd; the caller takes exclusive ownership.
    Ok(unsafe { LocalFd::from_raw(ret as i32) })
}
