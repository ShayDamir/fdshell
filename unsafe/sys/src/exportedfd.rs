#[repr(transparent)]
pub struct ExportedFd(i32);

impl ExportedFd {
    /// # Safety
    /// `raw` must be an open fd. Caller guarantees it stays valid for the value's lifetime.
    pub const unsafe fn from_raw(raw: i32) -> Self {
        Self(raw)
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
}
