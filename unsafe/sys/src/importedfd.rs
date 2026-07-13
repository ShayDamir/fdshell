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
    pub fn read(&self, buf: &mut [u8]) -> Result<isize, crate::SyscallError> {
        // SAFETY: `buf` is a valid mutable slice; `read` won't write past `buf.len()`.
        crate::cvt(unsafe { libc::read(self.0, buf.as_mut_ptr().cast(), buf.len()) })
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
    use super::*;

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
}
