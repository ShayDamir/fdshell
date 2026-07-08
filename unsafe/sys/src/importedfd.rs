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
    pub fn read(&self, buf: &mut [u8]) -> Result<isize, crate::SyscallError> {
        // SAFETY: `buf` is a valid mutable slice; `read` won't write past `buf.len()`.
        crate::cvt(unsafe {
            libc::read(
                self.0,
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                buf.len(),
            )
        })
    }

    /// Read from a raw fd number without verifying ownership or CLOEXEC.
    ///
    /// # Safety
    /// `raw` must be a valid open fd.
    pub fn read_from_raw(raw: i32, buf: &mut [u8]) -> Result<isize, crate::SyscallError> {
        // SAFETY: Caller guarantees `raw` is a valid open fd.
        // `buf` is a valid mutable slice; `read` won't write past `buf.len()`.
        crate::cvt(unsafe {
            libc::read(raw, buf.as_mut_ptr() as *mut core::ffi::c_void, buf.len())
        })
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
