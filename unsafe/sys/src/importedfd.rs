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
        let bytes = short
            .as_bytes()
            .change_context(crate::ImportedFdError::Never)?;
        Self::from_bytes(bytes)
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
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::ShortCStr;

    #[test]
    fn verify_internal_fd_rejected() {
        // Open /dev/null with O_CLOEXEC — CLOEXEC is set, so verify() must reject it.
        // SAFETY: `/dev/null` is a valid path; O_RDONLY|O_CLOEXEC are valid flags.
        let fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
        assert!(fd >= 0);
        let d = ImportedFd(fd);
        let result = d.verify();
        assert!(matches!(
            result,
            Err(ref e) if matches!(e.current_context(), crate::ImportedFdError::InternalFd)
        ));
        // SAFETY: `fd` is a valid open fd from the test above.
        unsafe { libc::close(fd) };
    }

    #[test]
    fn from_shortcstr_stdin() {
        let short = ShortCStr::from_vec(b"0".to_vec()).expect("test");
        let fd = ImportedFd::from_shortcstr(&short).expect("test");
        assert_eq!(fd.as_raw(), 0);
    }

    #[test]
    fn from_shortcstr_invalid_number() {
        let short = ShortCStr::from_vec(b"notanumber".to_vec()).expect("test");
        let result = ImportedFd::from_shortcstr(&short);
        assert!(matches!(
            result,
            Err(ref e) if matches!(e.current_context(), crate::ImportedFdError::NotANumber)
        ));
    }

    #[test]
    fn from_shortcstr_negative() {
        let short = ShortCStr::from_vec(b"-1".to_vec()).expect("test");
        let result = ImportedFd::from_shortcstr(&short);
        assert!(matches!(
            result,
            Err(ref e) if matches!(e.current_context(), crate::ImportedFdError::Negative)
        ));
    }

    #[test]
    fn from_shortcstr_internal_fd_rejected() {
        // Open /dev/null with O_CLOEXEC — CLOEXEC is set, so verify() must reject it.
        // SAFETY: `/dev/null` is a valid path; O_RDONLY|O_CLOEXEC are valid flags.
        let fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
        assert!(fd >= 0);
        let num = alloc::format!("{}", fd);
        let short = ShortCStr::from_vec(num.into_bytes()).expect("test");
        let result = ImportedFd::from_shortcstr(&short);
        assert!(matches!(
            result,
            Err(ref e) if matches!(e.current_context(), crate::ImportedFdError::InternalFd)
        ));
        // SAFETY: `fd` is a valid open fd from the test above.
        unsafe { libc::close(fd) };
    }
}
