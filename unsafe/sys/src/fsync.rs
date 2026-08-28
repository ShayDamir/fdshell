use crate::{LocalFd, SyscallError, cvt};

/// Flush the file open on `fd` to stable storage.
pub fn fsync(fd: &LocalFd) -> Result<(), SyscallError> {
    // SAFETY: `fd` is a valid open fd; `fsync` reads only the fd number, no
    // memory is dereferenced.
    cvt(unsafe { libc::fsync(fd.as_raw()) as isize })?;
    Ok(())
}
