use crate::{ImportedFd, LocalFd, SyscallError, cvt};

impl LocalFd {
    /// Preallocate (`mode` 0) or otherwise manage `len` bytes at `offset` on the
    /// file open on `self`. The file grows to `offset + len` when that exceeds
    /// its current size; `len` must be positive.
    pub fn fallocate(&self, mode: i32, offset: i64, len: i64) -> Result<(), SyscallError> {
        // SAFETY: `self` is a valid open fd; `fallocate` reads only the fd
        // number and the scalar arguments, no memory is dereferenced.
        cvt(unsafe { libc::fallocate(self.as_raw(), mode, offset, len) as isize })?;
        Ok(())
    }

    /// Advisory `flock(2)` lock on the open file description behind `self`.
    ///
    /// `operation` is a `LOCK_*` flag combination (LOCK_EX / LOCK_SH / LOCK_UN,
    /// optionally ORed with LOCK_NB). Blocking variants park until the lock is
    /// granted or a signal interrupts the syscall (EINTR).
    pub fn flock(&self, operation: i32) -> Result<(), SyscallError> {
        // SAFETY: `self` is a valid open fd; `flock` reads only the fd number
        // and the `operation` value, no memory is dereferenced.
        cvt(unsafe { libc::flock(self.as_raw(), operation) as isize })?;
        Ok(())
    }

    /// Shrink or extend the file open on `self` to exactly `length` bytes.
    pub fn ftruncate(&self, length: i64) -> Result<(), SyscallError> {
        // SAFETY: `self` is a valid open fd for writing; `ftruncate` reads
        // only the fd number and the `length` value, no memory is dereferenced.
        cvt(unsafe { libc::ftruncate(self.as_raw(), length) as isize })?;
        Ok(())
    }

    /// Flush the file open on `self` to stable storage.
    pub fn fsync(&self) -> Result<(), SyscallError> {
        // SAFETY: `self` is a valid open fd; `fsync` reads only the fd number,
        // no memory is dereferenced.
        cvt(unsafe { libc::fsync(self.as_raw()) as isize })?;
        Ok(())
    }

    pub fn fchdir(&self) -> Result<(), SyscallError> {
        // SAFETY: `fchdir` with an invalid fd returns `EBADF`.
        // It only modifies the calling process's CWD.
        cvt(unsafe { libc::fchdir(self.as_raw()) as isize })?;
        Ok(())
    }

    /// Zero-copy `copy_file_range(2)`: copy up to `count` bytes from `self` to
    /// `out`, advancing both file offsets. Returns the number of bytes actually
    /// copied (`0` means nothing to copy). When the fd pair cannot use the
    /// kernel copy (cross-filesystem `EXDEV`, a pipe, …) the kernel returns
    /// that errno so the caller can fall back to a read/write loop.
    pub fn copy_file_range_to(&self, out: &LocalFd, count: usize) -> Result<usize, SyscallError> {
        // off64_t offsets: passing `null_mut()` lets the kernel use and advance
        // each fd's current position. The count is `size_t`; the kernel caps it
        // to the bytes available at `self` and the space left in `out`.
        // SAFETY: `self` and `out` are valid open fds; `copy_file_range` reads
        // only the fd numbers and scalar arguments, no memory is dereferenced.
        cvt(unsafe {
            libc::copy_file_range(
                self.as_raw(),
                core::ptr::null_mut(),
                out.as_raw(),
                core::ptr::null_mut(),
                count,
                0,
            ) as isize
        })
        .map(|n| n as usize)
    }
}

impl ImportedFd {
    pub fn fchmod(&self, mode: u32) -> Result<(), crate::SyscallError> {
        // SAFETY: `fchmod` with an invalid fd returns `EBADF`.
        // `ImportedFd::verify()` guarantees the fd is open and non-CLOEXEC.
        // It only modifies the file permissions of an open fd.
        crate::cvt(unsafe { libc::fchmod(self.as_raw(), mode as libc::mode_t) as isize })?;
        Ok(())
    }
}
