#[repr(transparent)]
pub struct DupFd(i32);

impl DupFd {
    /// # Safety
    /// `raw` must be an open fd. Caller guarantees it stays valid for the value's lifetime.
    pub const unsafe fn from_raw(raw: i32) -> Self {
        Self(raw)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, i32> {
        let s = core::str::from_utf8(bytes).map_err(|_| crate::errno::EINVAL)?;
        let raw: i32 = s.parse().map_err(|_| crate::errno::EINVAL)?;
        let d = Self(raw);
        d.verify()?;
        Ok(d)
    }
    pub fn verify(&self) -> Result<(), i32> {
        let flags = crate::cvt(unsafe { libc::fcntl(self.0, libc::F_GETFD) as isize })?;
        if flags & libc::FD_CLOEXEC as isize != 0 {
            return Err(crate::errno::EINVAL);
        }
        Ok(())
    }
    pub fn as_raw(&self) -> i32 {
        self.0
    }
    pub fn at(&self) -> crate::AtFd<'_> {
        crate::AtFd::from(self)
    }
    /// Set CLOEXEC, converting this leaked DupFd into an owned Fd.
    pub fn into_owned(self) -> Result<crate::Fd, i32> {
        crate::cvt(unsafe { libc::fcntl(self.0, libc::F_SETFD, libc::FD_CLOEXEC) as isize })?;
        // SAFETY: fcntl atomically set CLOEXEC; caller gets exclusive ownership.
        Ok(unsafe { crate::Fd::from_raw(self.0) })
    }
}
