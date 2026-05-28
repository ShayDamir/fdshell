use crate::exportedfd::ExportedFd;

#[repr(transparent)]
pub struct LocalFd(i32);

impl LocalFd {
    /// # Safety
    /// `raw` must be a valid fd with CLOEXEC. Caller guarantees exclusive ownership.
    pub const unsafe fn from_raw(raw: i32) -> Self {
        Self(raw)
    }
    pub fn as_raw(&self) -> i32 {
        self.0
    }
    pub fn verify(&self) -> Result<(), i32> {
        let flags = crate::cvt(unsafe { libc::fcntl(self.0, libc::F_GETFD) as isize })?;
        if flags & libc::FD_CLOEXEC as isize == 0 {
            return Err(crate::errno::EINVAL);
        }
        Ok(())
    }
    pub fn try_close(self) -> Result<(), i32> {
        let raw = self.0;
        core::mem::forget(self);
        crate::cvt(unsafe { libc::close(raw) as isize })?;
        Ok(())
    }
    pub fn export(&self) -> Result<ExportedFd, i32> {
        let ret = crate::cvt(unsafe { libc::dup(self.0) as isize })?;
        // SAFETY: dup returns a valid fd or -1; cvt checked for errors.
        Ok(unsafe { ExportedFd::from_raw(ret as i32) })
    }
    pub fn export_to(&self, new: i32) -> Result<ExportedFd, i32> {
        let ret = crate::cvt(unsafe { libc::dup2(self.0, new) as isize })?;
        // SAFETY: export_to (dup2) always returns `new` on success (kernel contract).
        Ok(unsafe { ExportedFd::from_raw(ret as i32) })
    }
    pub fn try_clone(&self) -> Result<LocalFd, i32> {
        let ret = crate::cvt(unsafe { libc::fcntl(self.0, libc::F_DUPFD_CLOEXEC, 0) as isize })?;
        // SAFETY: `F_DUPFD_CLOEXEC` returns a new fd with CLOEXEC atomically set.
        Ok(unsafe { LocalFd::from_raw(ret as i32) })
    }
    pub fn try_clone_to(&self, new: i32) -> Result<LocalFd, i32> {
        let ret = crate::cvt(unsafe { libc::dup3(self.0, new, libc::O_CLOEXEC) as isize })?;
        // SAFETY: dup3 returns `new` on success with CLOEXEC atomically set.
        Ok(unsafe { LocalFd::from_raw(ret as i32) })
    }
    pub fn at(&self) -> crate::AtFd<'_> {
        crate::AtFd::from(self)
    }
}

impl Drop for LocalFd {
    fn drop(&mut self) {
        unsafe { libc::close(self.0) };
    }
}
