use crate::ImportedFd;

pub fn fchmod(fd: &ImportedFd, mode: u32) -> Result<(), crate::SyscallError> {
    // SAFETY: `fchmod` with an invalid fd returns `EBADF`.
    // `ImportedFd::verify()` guarantees the fd is open and non-CLOEXEC.
    // It only modifies the file permissions of an open fd.
    crate::cvt(unsafe { libc::fchmod(fd.as_raw(), mode as libc::mode_t) as isize })?;
    Ok(())
}
