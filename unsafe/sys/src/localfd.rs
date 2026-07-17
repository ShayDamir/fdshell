use crate::exportedfd::ExportedFd;
use core::fmt;

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
    pub fn verify(&self) -> Result<(), crate::SyscallError> {
        // SAFETY: `self.0` is a valid fd by `LocalFd` invariant; fcntl
        // with invalid fd returns -1/EBADF, caught by `cvt`.
        let flags = crate::cvt(unsafe { libc::fcntl(self.0, libc::F_GETFD) as isize })?;
        if flags & libc::FD_CLOEXEC as isize == 0 {
            return Err(crate::SyscallError::EINVAL("fcntl(F_GETFD)"));
        }
        Ok(())
    }
    pub fn try_close(self) -> Result<(), crate::SyscallError> {
        let raw = self.0;
        core::mem::forget(self);
        // SAFETY: `raw` is a valid fd (from `self.0`, forgotten to
        // prevent double-close); close of a bad fd is a no-op.
        crate::cvt(unsafe { libc::close(raw) as isize })?;
        Ok(())
    }
    pub fn export(&self) -> Result<ExportedFd, crate::SyscallError> {
        // SAFETY: `self.0` is a valid open fd; `dup` returns a new
        // fd or -1 on error, checked by `cvt`.
        let ret = crate::cvt(unsafe { libc::dup(self.0) as isize })?;
        // SAFETY: dup returns a valid fd or -1; cvt checked for errors.
        Ok(unsafe { ExportedFd::from_raw(ret as i32) })
    }
    pub fn export_to(&self, new: i32) -> Result<ExportedFd, crate::SyscallError> {
        // SAFETY: `self.0` is a valid open fd; `dup2` on invalid fd
        // safely returns -1/EBADF, handled by `cvt`.
        let ret = crate::cvt(unsafe { libc::dup2(self.0, new) as isize })?;
        // SAFETY: export_to (dup2) always returns `new` on success (kernel contract).
        Ok(unsafe { ExportedFd::from_raw(ret as i32) })
    }
    pub fn try_clone(&self) -> Result<LocalFd, crate::SyscallError> {
        // SAFETY: `self.0` is a valid open fd; `F_DUPFD_CLOEXEC` with
        // an invalid fd safely returns -1/EBADF.
        let ret = crate::cvt(unsafe { libc::fcntl(self.0, libc::F_DUPFD_CLOEXEC, 0) as isize })?;
        // SAFETY: `F_DUPFD_CLOEXEC` returns a new fd with CLOEXEC atomically set.
        Ok(unsafe { LocalFd::from_raw(ret as i32) })
    }
    pub fn try_clone_above(&self, min_fd: i32) -> Result<LocalFd, crate::SyscallError> {
        // SAFETY: `self.0` is a valid open fd; `F_DUPFD_CLOEXEC` with
        // `min_fd` ensures the new fd is >= min_fd, avoiding collisions.
        let ret =
            crate::cvt(unsafe { libc::fcntl(self.0, libc::F_DUPFD_CLOEXEC, min_fd) as isize })?;
        // SAFETY: `F_DUPFD_CLOEXEC` returns a new fd >= min_fd with CLOEXEC set.
        Ok(unsafe { LocalFd::from_raw(ret as i32) })
    }
    pub fn try_clone_to(&self, new: i32) -> Result<LocalFd, crate::SyscallError> {
        // SAFETY: `self.0` is a valid open fd; `dup3` with invalid
        // args safely returns -1, handled by `cvt`.
        let ret = crate::cvt(unsafe { libc::dup3(self.0, new, libc::O_CLOEXEC) as isize })?;
        // SAFETY: dup3 returns `new` on success with CLOEXEC atomically set.
        Ok(unsafe { LocalFd::from_raw(ret as i32) })
    }
    pub fn at(&self) -> crate::AtFd<'_> {
        crate::AtFd::from(self)
    }
    pub fn read(&self, buf: &mut [u8]) -> Result<isize, crate::SyscallError> {
        // SAFETY: `buf` is a valid mutable slice; `read` won't write past `buf.len()`.
        crate::cvt(unsafe { libc::read(self.as_raw(), buf.as_mut_ptr().cast(), buf.len()) })
    }

    /// Read until EOF or buffer full.
    pub fn read_all(&self, buf: &mut [u8]) -> Result<usize, crate::SyscallError> {
        let mut offset = 0;
        loop {
            let slice = buf
                .get_mut(offset..)
                .ok_or(crate::SyscallError::EINVAL("buffer full"))?;
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
        unsafe { libc::close(self.0) };
    }
}
