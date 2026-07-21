//! Imported file descriptor — wraps a non-owned fd (e.g. inherited from parent).

use core::fmt;

use error_stack::{Report, ResultExt, ensure};

use crate::ShortCStr;

#[repr(transparent)]
pub struct ImportedFd(i32);

impl fmt::Display for ImportedFd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ImportedFd {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Report<crate::ImportedFdError>> {
        let s = core::str::from_utf8(bytes).change_context(crate::ImportedFdError::NotANumber)?;
        let raw: i32 = s
            .parse()
            .change_context(crate::ImportedFdError::NotANumber)?;
        ensure!(raw >= 0, crate::ImportedFdError::Negative);
        let d = Self(raw);
        d.verify().map(|_| d)
    }

    pub fn from_shortcstr(
        short: &crate::ShortCStr,
    ) -> Result<Self, Report<crate::ImportedFdError>> {
        let raw: i32 = short
            .parse()
            .change_context(crate::ImportedFdError::NotANumber)?;
        ensure!(raw >= 0, crate::ImportedFdError::Negative);
        let d = Self(raw);
        d.verify().map(|_| d)
    }

    pub fn verify(&self) -> Result<(), Report<crate::ImportedFdError>> {
        // SAFETY: `self.0` is a valid fd by `ImportedFd` invariant;
        // fcntl on invalid fd returns -1/EBADF safely.
        let flags = crate::cvt(unsafe { libc::fcntl(self.0, libc::F_GETFD) as isize })
            .change_context(crate::ImportedFdError::GetFlags)?;
        // CLOEXEC set means the fd was created internally (e.g. by the shell)
        // and was never passed from a caller. ImportedFd represents borrowed
        // fds from external sources — they must survive exec, which requires
        // CLOEXEC to be clear.
        ensure!(
            flags & libc::FD_CLOEXEC as isize == 0,
            crate::ImportedFdError::InternalFd
        );
        Ok(())
    }

    pub fn as_raw(&self) -> i32 {
        self.0
    }

    /// Construct from a raw fd without verification.
    ///
    /// # Safety
    /// `fd` must be a valid open fd with CLOEXEC clear.
    pub const unsafe fn from_raw(fd: i32) -> Self {
        Self(fd)
    }

    pub fn at(&self) -> crate::AtFd<'_> {
        crate::AtFd::from(self)
    }

    /// Set CLOEXEC, converting this imported fd into a local owned fd.
    pub fn try_into_local(self) -> Result<crate::LocalFd, Report<crate::ImportedFdError>> {
        // SAFETY: `self.0` is a valid open fd; fcntl F_SETFD on
        // an invalid fd returns -1, handled by `cvt`.
        crate::cvt(unsafe { libc::fcntl(self.0, libc::F_SETFD, libc::FD_CLOEXEC) as isize })
            .change_context(crate::ImportedFdError::SetFlags)?;
        // SAFETY: fcntl atomically set CLOEXEC; caller gets exclusive ownership.
        Ok(unsafe { crate::LocalFd::from_raw(self.0) })
    }

    /// Read bytes from the fd into buf.
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, crate::SyscallError> {
        // SAFETY: `buf` is a valid mutable slice; `read` won't write past `buf.len()`.
        crate::cvt(unsafe { libc::read(self.as_raw(), buf.as_mut_ptr().cast(), buf.len()) })
            .map(|n| n as usize)
    }

    /// Write bytes to the fd.
    pub fn write(&self, buf: &[u8]) -> Result<usize, crate::SyscallError> {
        // SAFETY: `buf` is a valid slice; `write` won't write past `buf.len()`.
        crate::cvt(unsafe { libc::write(self.as_raw(), buf.as_ptr().cast(), buf.len()) })
            .map(|n| n as usize)
    }

    /// Write all bytes, retrying on short writes.
    pub fn write_all(&self, buf: &[u8]) -> Result<(), crate::SyscallError> {
        let mut written = 0;
        while written < buf.len() {
            let slice = buf.get(written..).ok_or(crate::SyscallError::Never)?;
            let n = self.write(slice)?;
            if n == 0 {
                break;
            }
            written += n;
        }
        Ok(())
    }

    /// Write a ShortCStr to the fd.
    pub fn write_str(&self, s: &ShortCStr) -> Result<(), Report<crate::SyscallError>> {
        Ok(self.write_all(s.as_bytes().change_context(crate::SyscallError::Never)?)?)
    }

    /// Read until EOF or buffer full.
    pub fn read_all(&self, buf: &mut [u8]) -> Result<usize, crate::SyscallError> {
        let mut offset = 0;
        loop {
            let slice = buf.get_mut(offset..).ok_or(crate::SyscallError::Never)?;
            match self.read(slice)? {
                0 => break,
                n => offset += n,
            }
            if offset >= buf.len() {
                break;
            }
        }
        Ok(offset)
    }
}
