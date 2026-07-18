use crate::exportedfd::ExportedFd;
use crate::{AtFd, LocalFdError, SyscallError, cvt};
use core::fmt;

use error_stack::{Report, ResultExt, ensure};

use libc::{close, dup, dup2, dup3, fcntl, read};

use crate::fcntl::{F_DUPFD_CLOEXEC, F_GETFD, FD_CLOEXEC, O_CLOEXEC};

#[repr(transparent)]
pub struct LocalFd(i32);

impl fmt::Display for LocalFd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl LocalFd {
    /// # Safety
    /// `raw` must be a valid fd with CLOEXEC. Caller guarantees exclusive ownership.
    pub const unsafe fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    pub fn as_raw(&self) -> i32 {
        self.0
    }

    pub fn verify(&self) -> Result<(), Report<LocalFdError>> {
        // SAFETY: `self.0` is a valid fd by `LocalFd` invariant; fcntl
        // with invalid fd returns -1/EBADF, caught by `cvt`.
        let flags = cvt(unsafe { fcntl(self.0, F_GETFD) as isize })
            .change_context(LocalFdError::GetFlags)?;
        ensure!(flags & FD_CLOEXEC as isize != 0, LocalFdError::NoCloexec);
        Ok(())
    }

    pub fn try_close(self) -> Result<(), SyscallError> {
        let raw = self.0;
        core::mem::forget(self);
        // SAFETY: `raw` is a valid fd (from `self.0`, forgotten to
        // prevent double-close); close of a bad fd is a no-op.
        cvt(unsafe { close(raw) as isize })?;
        Ok(())
    }

    pub fn export(&self) -> Result<ExportedFd, SyscallError> {
        // SAFETY: `self.0` is a valid open fd; `dup` returns a new
        // fd or -1 on error, checked by `cvt`.
        let ret = cvt(unsafe { dup(self.0) as isize })?;
        // SAFETY: dup returns a valid fd or -1; cvt checked for errors.
        Ok(unsafe { ExportedFd::from_raw(ret as i32) })
    }

    pub fn export_to(&self, new: i32) -> Result<ExportedFd, SyscallError> {
        // SAFETY: `self.0` is a valid open fd; `dup2` on invalid fd
        // safely returns -1/EBADF, handled by `cvt`.
        let ret = cvt(unsafe { dup2(self.0, new) as isize })?;
        // SAFETY: export_to (dup2) always returns `new` on success (kernel contract).
        Ok(unsafe { ExportedFd::from_raw(ret as i32) })
    }

    pub fn try_clone(&self) -> Result<LocalFd, SyscallError> {
        // SAFETY: `self.0` is a valid open fd; `F_DUPFD_CLOEXEC` with
        // an invalid fd safely returns -1/EBADF.
        let ret = cvt(unsafe { fcntl(self.0, F_DUPFD_CLOEXEC, 0) as isize })?;
        // SAFETY: `F_DUPFD_CLOEXEC` returns a new fd with CLOEXEC atomically set.
        Ok(unsafe { LocalFd::from_raw(ret as i32) })
    }

    pub fn try_clone_above(&self, min_fd: i32) -> Result<LocalFd, SyscallError> {
        // SAFETY: `self.0` is a valid open fd; `F_DUPFD_CLOEXEC` with
        // `min_fd` ensures the new fd is >= min_fd, avoiding collisions.
        let ret = cvt(unsafe { fcntl(self.0, F_DUPFD_CLOEXEC, min_fd) as isize })?;
        // SAFETY: `F_DUPFD_CLOEXEC` returns a new fd >= min_fd with CLOEXEC set.
        Ok(unsafe { LocalFd::from_raw(ret as i32) })
    }

    pub fn try_clone_to(&self, new: i32) -> Result<LocalFd, SyscallError> {
        // SAFETY: `self.0` is a valid open fd; `dup3` with invalid
        // args safely returns -1, handled by `cvt`.
        let ret = cvt(unsafe { dup3(self.0, new, O_CLOEXEC) as isize })?;
        // SAFETY: dup3 returns `new` on success with CLOEXEC atomically set.
        Ok(unsafe { LocalFd::from_raw(ret as i32) })
    }

    pub fn at(&self) -> AtFd<'_> {
        AtFd::from(self)
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<isize, SyscallError> {
        // SAFETY: `buf` is a valid mutable slice; `read` won't write past `buf.len()`.
        cvt(unsafe { read(self.as_raw(), buf.as_mut_ptr().cast(), buf.len()) })
    }

    /// Read until EOF or buffer full.
    pub fn read_all(&self, buf: &mut [u8]) -> Result<usize, SyscallError> {
        let mut offset = 0;
        loop {
            let slice = buf
                .get_mut(offset..)
                .ok_or(SyscallError::EINVAL("buffer full"))?;
            match self.read(slice)? {
                0 => break,
                n => offset += n as usize,
            }
            if offset >= buf.len() {
                break;
            }
        }
        Ok(offset)
    }
}

impl Drop for LocalFd {
    fn drop(&mut self) {
        // SAFETY: `self.0` is a valid fd by `LocalFd` invariant;
        // dropping the value transfers ownership to the kernel.
        unsafe { close(self.0) };
    }
}
