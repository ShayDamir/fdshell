use crate::LocalFd;

pub fn fchdir(fd: &LocalFd) -> Result<(), crate::SyscallError> {
    // SAFETY: `fchdir` with an invalid fd returns `EBADF`.
    // It only modifies the calling process's CWD.
    crate::cvt(unsafe { libc::fchdir(fd.as_raw()) as isize })?;
    Ok(())
}
