//! Imported file descriptor — wraps a non-owned fd (e.g. inherited from parent).

use core::fmt;

use error_stack::{Report, ResultExt, ensure};

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

    /// Validate an fd number parsed from a script (e.g. the `1` in `2>&1`).
    pub fn from_number(raw: i32) -> Result<Self, Report<crate::ImportedFdError>> {
        ensure!(raw >= 0, crate::ImportedFdError::Negative);
        let d = Self(raw);
        d.verify().map(|_| d)
    }

    /// Duplicate this borrowed fd into a new owned fd with `CLOEXEC` set,
    /// leaving the original untouched.
    pub fn try_dup(&self) -> Result<crate::LocalFd, Report<crate::ImportedFdError>> {
        // SAFETY: `self.0` is a valid open fd; `F_DUPFD_CLOEXEC` on an invalid
        // fd safely returns -1/EBADF, handled by `cvt`.
        let ret = crate::cvt(unsafe { libc::fcntl(self.0, libc::F_DUPFD_CLOEXEC, 0) as isize })
            .change_context(crate::ImportedFdError::SetFlags)?;
        // SAFETY: `F_DUPFD_CLOEXEC` returns a new fd >= 0 with CLOEXEC atomically set.
        Ok(unsafe { crate::LocalFd::from_raw(ret as i32) })
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

    /// Returns `true` if this fd refers to a terminal.
    pub fn is_tty(&self) -> Result<bool, crate::SyscallError> {
        // SAFETY: `isatty` only inspects an open fd; the `ImportedFd`
        // invariant guarantees the fd is open.
        let ret = crate::cvt(unsafe { libc::isatty(self.0) as isize })?;
        Ok(ret != 0)
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
}

mod rw;
