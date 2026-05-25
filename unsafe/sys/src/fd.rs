#[repr(transparent)]
pub struct Fd(i32);

impl Fd {
    /// # Safety
    /// `raw` must be a valid fd with `O_CLOEXEC` or equivalent.
    /// The caller guarantees exclusive ownership — no other code will close it.
    pub const unsafe fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    pub fn as_raw(&self) -> i32 {
        self.0
    }

    pub fn verify(&self) -> bool {
        let ret = unsafe { libc::fcntl(self.0, libc::F_GETFD) };
        ret >= 0 && (ret & libc::FD_CLOEXEC) != 0
    }

    /// Explicit close with error reporting. Silently closes on Drop otherwise.
    pub fn close(self) -> Result<(), i32> {
        let raw = self.0;
        core::mem::forget(self);
        // SAFETY: `raw` came from a valid fd (constructor guarantee).
        crate::cvt(unsafe { libc::close(raw) as isize })?;
        Ok(())
    }

    /// Duplicate to the lowest available fd. Result is not CLOEXEC.
    pub fn dup(&self) -> Result<DupFd, i32> {
        // SAFETY: `self.0` is a valid fd; dup returns lowest available or -1.
        let ret = crate::cvt(unsafe { libc::dup(self.0) as isize })?;
        Ok(DupFd(ret as i32))
    }

    /// Duplicate to a specific fd number. Result is not CLOEXEC.
    pub fn dup2(&self, new: DupFd) -> Result<DupFd, i32> {
        // SAFETY: `self.0` is a valid fd; invalid `new` returns EBADF.
        let ret = crate::cvt(unsafe { libc::dup2(self.0, new.as_raw()) as isize })?;
        Ok(DupFd(ret as i32))
    }

    /// Duplicate to a specific fd number WITH CLOEXEC. Result is owned.
    pub fn dup3(&self, new: i32) -> Result<Fd, i32> {
        // SAFETY: `self.0` is a valid fd; O_CLOEXEC sets CLOEXEC on the copy.
        let ret = crate::cvt(unsafe { libc::dup3(self.0, new, libc::O_CLOEXEC) as isize })?;
        // SAFETY: `ret` is a valid fd with CLOEXEC set by dup3.
        Ok(unsafe { Fd::from_raw(ret as i32) })
    }
}

impl Drop for Fd {
    fn drop(&mut self) {
        // SAFETY: `self.0` is a valid fd; errors silently ignored in Drop.
        unsafe {
            libc::close(self.0);
        }
    }
}

/// Non-owned fd — no CLOEXEC guarantee, no Drop. Created by `dup`/`dup2`
/// for child-process inheritance. Lifetime managed by the kernel.
#[repr(transparent)]
pub struct DupFd(i32);

impl DupFd {
    /// # Safety
    /// `raw` must be an open fd number that stays valid for the lifetime of this value.
    pub const unsafe fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, i32> {
        let s = core::str::from_utf8(bytes).map_err(|_| crate::errno::EINVAL)?;
        let raw: i32 = s.parse().map_err(|_| crate::errno::EINVAL)?;
        // SAFETY: fcntl(F_GETFD) returns ≥0 if fd is open, -1 + errno otherwise.
        crate::cvt(unsafe { libc::fcntl(raw, libc::F_GETFD) as isize })?;
        // SAFETY: fd `raw` is confirmed open by F_GETFD above.
        Ok(Self(raw))
    }

    pub fn as_raw(&self) -> i32 {
        self.0
    }

    pub fn at(&self) -> crate::AtFd<'_> {
        crate::AtFd::from(self)
    }
}

impl Fd {
    pub fn at(&self) -> crate::AtFd<'_> {
        crate::AtFd::from(self)
    }
}
