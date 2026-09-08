use crate::{ImportedFd, LocalFd, SyscallError, cvt};

/// Preallocate (`mode` 0) or otherwise manage `len` bytes at `offset` on the
/// file open on `fd`. The file grows to `offset + len` when that exceeds its
/// current size; `len` must be positive.
pub fn fallocate(fd: &LocalFd, mode: i32, offset: i64, len: i64) -> Result<(), SyscallError> {
    // SAFETY: `fd` is a valid open fd; `fallocate` reads only the fd number and
    // the scalar arguments, no memory is dereferenced.
    cvt(unsafe { libc::fallocate(fd.as_raw(), mode, offset, len) as isize })?;
    Ok(())
}

pub fn fchmod(fd: &ImportedFd, mode: u32) -> Result<(), crate::SyscallError> {
    // SAFETY: `fchmod` with an invalid fd returns `EBADF`.
    // `ImportedFd::verify()` guarantees the fd is open and non-CLOEXEC.
    // It only modifies the file permissions of an open fd.
    crate::cvt(unsafe { libc::fchmod(fd.as_raw(), mode as libc::mode_t) as isize })?;
    Ok(())
}

/// Advisory `flock(2)` lock on the open file description behind `fd`.
///
/// `operation` is a `LOCK_*` flag combination (LOCK_EX / LOCK_SH / LOCK_UN,
/// optionally ORed with LOCK_NB). Blocking variants park until the lock is
/// granted or a signal interrupts the syscall (EINTR).
pub fn flock(fd: &LocalFd, operation: i32) -> Result<(), SyscallError> {
    // SAFETY: `fd` is a valid open fd; `flock` reads only the fd number and the
    // `operation` value, no memory is dereferenced.
    cvt(unsafe { libc::flock(fd.as_raw(), operation) as isize })?;
    Ok(())
}

/// Shrink or extend the file open on `fd` to exactly `length` bytes.
pub fn ftruncate(fd: &LocalFd, length: i64) -> Result<(), SyscallError> {
    // SAFETY: `fd` is a valid open fd for writing; `ftruncate` reads only the
    // fd number and the `length` value, no memory is dereferenced.
    cvt(unsafe { libc::ftruncate(fd.as_raw(), length) as isize })?;
    Ok(())
}

/// Flush the file open on `fd` to stable storage.
pub fn fsync(fd: &LocalFd) -> Result<(), SyscallError> {
    // SAFETY: `fd` is a valid open fd; `fsync` reads only the fd number, no
    // memory is dereferenced.
    cvt(unsafe { libc::fsync(fd.as_raw()) as isize })?;
    Ok(())
}

/// Zero-copy `copy_file_range(2)`: copy up to `count` bytes from `in_fd` to
/// `out_fd`, advancing both file offsets. Returns the number of bytes actually
/// copied (`0` means nothing to copy). When the fd pair cannot use the kernel
/// copy (cross-filesystem `EXDEV`, a pipe, …) the kernel returns that errno so
/// the caller can fall back to a read/write loop.
pub fn copy_file_range(
    in_fd: &LocalFd,
    out_fd: &LocalFd,
    count: usize,
) -> Result<usize, SyscallError> {
    // off64_t offsets: passing `null_mut()` lets the kernel use and advance each
    // fd's current position. The count is `size_t`; the kernel caps it to the
    // bytes available at `in_fd` and the space left in `out_fd`.
    // SAFETY: `in_fd` and `out_fd` are valid open fds; `copy_file_range` reads
    // only the fd numbers and scalar arguments, no memory is dereferenced.
    cvt(unsafe {
        libc::copy_file_range(
            in_fd.as_raw(),
            core::ptr::null_mut(),
            out_fd.as_raw(),
            core::ptr::null_mut(),
            count,
            0,
        ) as isize
    })
    .map(|n| n as usize)
}
