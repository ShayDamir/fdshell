use crate::{LocalFd, SyscallError, cvt};

/// Preallocate (`mode` 0) or otherwise manage `len` bytes at `offset` on the
/// file open on `fd`. The file grows to `offset + len` when that exceeds its
/// current size; `len` must be positive.
pub fn fallocate(fd: &LocalFd, mode: i32, offset: i64, len: i64) -> Result<(), SyscallError> {
    // SAFETY: `fd` is a valid open fd; `fallocate` reads only the fd number and
    // the scalar arguments, no memory is dereferenced.
    cvt(unsafe { libc::fallocate(fd.as_raw(), mode, offset, len) as isize })?;
    Ok(())
}
