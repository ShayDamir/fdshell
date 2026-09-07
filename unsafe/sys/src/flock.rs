use crate::{LocalFd, SyscallError, cvt};

/// Take or release an advisory `flock(2)` lock on the open file description
/// of `fd`. `operation` is `LOCK_EX`, `LOCK_SH`, or `LOCK_UN`; OR in
/// `LOCK_NB` to fail with `EWOULDBLOCK` instead of blocking.
pub fn flock(fd: &LocalFd, operation: i32) -> Result<(), SyscallError> {
    // SAFETY: `fd` is a valid open fd; `flock` reads only the fd number and
    // the scalar `operation`, no memory is dereferenced.
    cvt(unsafe { libc::flock(fd.as_raw(), operation) as isize })?;
    Ok(())
}
