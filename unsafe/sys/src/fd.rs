use crate::dupfd::DupFd;

#[repr(transparent)]
pub struct Fd(i32);

impl Fd {
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
    pub fn close(self) -> Result<(), i32> {
        let raw = self.0;
        core::mem::forget(self);
        crate::cvt(unsafe { libc::close(raw) as isize })?;
        Ok(())
    }
    pub fn dup(&self) -> Result<DupFd, i32> {
        let ret = crate::cvt(unsafe { libc::dup(self.0) as isize })?;
        // SAFETY: dup returns a valid fd or -1; cvt checked for errors.
        Ok(unsafe { DupFd::from_raw(ret as i32) })
    }
    pub fn dup_to(&self, new: i32) -> Result<DupFd, i32> {
        let ret = crate::cvt(unsafe { libc::dup2(self.0, new) as isize })?;
        // SAFETY: dup_to (dup2) always returns `new` on success (kernel contract).
        Ok(unsafe { DupFd::from_raw(ret as i32) })
    }
    pub fn try_clone_any(&self) -> Result<Fd, i32> {
        let ret = crate::cvt(unsafe { libc::dup(self.0) as isize })?;
        let raw = ret as i32;
        crate::cvt(unsafe { libc::fcntl(raw, libc::F_SETFD, libc::FD_CLOEXEC) as isize })?;
        // SAFETY: `raw` has CLOEXEC confirmed above.
        Ok(unsafe { Fd::from_raw(raw) })
    }
    pub fn try_clone(&self, new: i32) -> Result<Fd, i32> {
        let ret = crate::cvt(unsafe { libc::dup3(self.0, new, libc::O_CLOEXEC) as isize })?;
        Ok(unsafe { Fd::from_raw(ret as i32) })
    }
    pub fn at(&self) -> crate::AtFd<'_> {
        crate::AtFd::from(self)
    }
}

impl Drop for Fd {
    fn drop(&mut self) {
        unsafe { libc::close(self.0) };
    }
}
