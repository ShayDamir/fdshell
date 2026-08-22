use crate::{SyscallError, cvt};

/// Close a process fd by number.
///
/// The caller must own the fd (be about to drop it) or be deliberately
/// closing it, e.g. an `N>&-` redirection dropping the shell's own fd.
pub fn close(fd: i32) -> Result<(), SyscallError> {
    // SAFETY: caller guarantees exclusive ownership of `fd`; `close` on an
    // invalid or already-closed fd safely returns -1/EBADF, caught by `cvt`.
    cvt(unsafe { libc::close(fd) as isize }).map(|_| ())
}
