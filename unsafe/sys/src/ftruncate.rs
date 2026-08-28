use crate::{LocalFd, SyscallError, cvt};

/// Shrink or extend the file open on `fd` to exactly `length` bytes.
pub fn ftruncate(fd: &LocalFd, length: i64) -> Result<(), SyscallError> {
    // SAFETY: `fd` is a valid open fd for writing; `ftruncate` reads only the
    // fd number and the `length` value, no memory is dereferenced.
    cvt(unsafe { libc::ftruncate(fd.as_raw(), length) as isize })?;
    Ok(())
}
