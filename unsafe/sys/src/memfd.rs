use crate::{LocalFd, SyscallError, cvt};

/// Create an anonymous in-memory file with `CLOEXEC` set.
pub fn memfd_create() -> Result<LocalFd, SyscallError> {
    // SAFETY: `memfd_create` writes a new fd on success or -1 on error,
    // checked by `cvt`; the name is a valid C string.
    let ret = cvt(unsafe { libc::memfd_create(c"memfd".as_ptr(), libc::MFD_CLOEXEC) as isize })?;
    // SAFETY: `MFD_CLOEXEC` is set, satisfying the `LocalFd` invariant.
    Ok(unsafe { LocalFd::from_raw(ret as i32) })
}
