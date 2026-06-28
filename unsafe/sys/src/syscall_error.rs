//! Typed syscall error — bridges raw errno to the type system.
//!
//! `SyscallError` replaces raw `i32` errnos in `unsafe/sys/` return types so that
//! callers can use `error_stack::ResultExt::change_context()` instead of ad-hoc
//! `From<i32>` conversions.

use core::fmt;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum SyscallError {
    E2BIG,
    EAGAIN,
    EBADF,
    EEXIST,
    EINVAL,
    EIO,
    EMFILE,
    ENOENT,
    ENOSYS,
    EPERM,
    Other(i32),
}

impl SyscallError {
    pub fn errno(self) -> i32 {
        use SyscallError::*;
        match self {
            E2BIG => libc::E2BIG,
            EAGAIN => libc::EAGAIN,
            EBADF => libc::EBADF,
            EEXIST => libc::EEXIST,
            EINVAL => libc::EINVAL,
            EIO => libc::EIO,
            EMFILE => libc::EMFILE,
            ENOENT => libc::ENOENT,
            ENOSYS => libc::ENOSYS,
            EPERM => libc::EPERM,
            Other(n) => n,
        }
    }
}

impl fmt::Display for SyscallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SyscallError::*;
        let name = match self {
            E2BIG => "E2BIG",
            EAGAIN => "EAGAIN",
            EBADF => "EBADF",
            EEXIST => "EEXIST",
            EINVAL => "EINVAL",
            EIO => "EIO",
            EMFILE => "EMFILE",
            ENOENT => "ENOENT",
            ENOSYS => "ENOSYS",
            EPERM => "EPERM",
            Other(n) => return write!(f, "errno {}", n),
        };
        write!(f, "{}", name)
    }
}

impl core::error::Error for SyscallError {}

impl From<i32> for SyscallError {
    fn from(raw: i32) -> Self {
        use SyscallError::*;
        match raw {
            libc::E2BIG => E2BIG,
            libc::EAGAIN => EAGAIN,
            libc::EBADF => EBADF,
            libc::EEXIST => EEXIST,
            libc::EINVAL => EINVAL,
            libc::EIO => EIO,
            libc::EMFILE => EMFILE,
            libc::ENOENT => ENOENT,
            libc::ENOSYS => ENOSYS,
            libc::EPERM => EPERM,
            n => Other(n),
        }
    }
}
