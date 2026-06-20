use core::convert::TryFrom;
use core::ffi::CStr;

use crate::shortcstr::ShortCStr;

#[repr(transparent)]
pub struct ImportedFd(i32);

impl ImportedFd {
    /// # Safety
    /// `raw` must be an open fd. Caller guarantees it stays valid for the value's lifetime.
    pub const unsafe fn from_raw(raw: i32) -> Self {
        Self(raw)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::SyscallError> {
        let s = core::str::from_utf8(bytes).map_err(|_| crate::SyscallError::EINVAL)?;
        let raw: i32 = s.parse().map_err(|_| crate::SyscallError::EINVAL)?;
        let d = Self(raw);
        d.verify()?;
        Ok(d)
    }
    pub fn verify(&self) -> Result<(), crate::SyscallError> {
        // SAFETY: `self.0` is a valid fd by `ImportedFd` invariant;
        // fcntl on invalid fd returns -1/EBADF safely.
        let flags = crate::cvt(unsafe { libc::fcntl(self.0, libc::F_GETFD) as isize })?;
        if flags & libc::FD_CLOEXEC as isize != 0 {
            return Err(crate::SyscallError::EINVAL);
        }
        Ok(())
    }
    pub fn as_raw(&self) -> i32 {
        self.0
    }
    pub fn at(&self) -> crate::AtFd<'_> {
        crate::AtFd::from(self)
    }
    /// Set CLOEXEC, converting this imported fd into a local owned fd.
    pub fn try_into_local(self) -> Result<crate::LocalFd, crate::SyscallError> {
        // SAFETY: `self.0` is a valid open fd; fcntl F_SETFD on
        // an invalid fd returns -1, handled by `cvt`.
        crate::cvt(unsafe { libc::fcntl(self.0, libc::F_SETFD, libc::FD_CLOEXEC) as isize })?;
        // SAFETY: fcntl atomically set CLOEXEC; caller gets exclusive ownership.
        Ok(unsafe { crate::LocalFd::from_raw(self.0) })
    }
}

impl TryFrom<&ShortCStr> for ImportedFd {
    type Error = crate::SyscallError;
    fn try_from(scs: &ShortCStr) -> Result<Self, crate::SyscallError> {
        Self::from_bytes(scs.as_bytes()?)
    }
}

impl TryFrom<&CStr> for ImportedFd {
    type Error = crate::SyscallError;
    fn try_from(s: &CStr) -> Result<Self, crate::SyscallError> {
        Self::from_bytes(s.to_bytes())
    }
}
