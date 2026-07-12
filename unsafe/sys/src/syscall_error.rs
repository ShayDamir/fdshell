//! Typed syscall error — bridges raw errno to the type system.
//!
//! `SyscallError` replaces raw `i32` errnos in `unsafe/sys/` return types so that
//! callers can use `error_stack::ResultExt::change_context()` instead of ad-hoc
//! `From<i32>` conversions.

use core::fmt;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum SyscallError {
    E2BIG(&'static str),
    EAGAIN(&'static str),
    EBADF(&'static str),
    EEXIST(&'static str),
    EINVAL(&'static str),
    EIO(&'static str),
    EMFILE(&'static str),
    ENOENT(&'static str),
    ENOSYS(&'static str),
    EPERM(&'static str),
    Other { errno: i32, syscall: &'static str },
}

impl SyscallError {
    pub fn errno(self) -> i32 {
        use SyscallError::*;
        match self {
            E2BIG(_) => libc::E2BIG,
            EAGAIN(_) => libc::EAGAIN,
            EBADF(_) => libc::EBADF,
            EEXIST(_) => libc::EEXIST,
            EINVAL(_) => libc::EINVAL,
            EIO(_) => libc::EIO,
            EMFILE(_) => libc::EMFILE,
            ENOENT(_) => libc::ENOENT,
            ENOSYS(_) => libc::ENOSYS,
            EPERM(_) => libc::EPERM,
            Other { errno, .. } => errno,
        }
    }

    pub fn syscall(self) -> &'static str {
        use SyscallError::*;
        match self {
            E2BIG(s) | EAGAIN(s) | EBADF(s) | EEXIST(s) | EINVAL(s) | EIO(s) | EMFILE(s)
            | ENOENT(s) | ENOSYS(s) | EPERM(s) => s,
            Other { syscall, .. } => syscall,
        }
    }
}

impl fmt::Display for SyscallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SyscallError::*;
        let (name, syscall) = match self {
            E2BIG(s) => ("E2BIG", s),
            EAGAIN(s) => ("EAGAIN", s),
            EBADF(s) => ("EBADF", s),
            EEXIST(s) => ("EEXIST", s),
            EINVAL(s) => ("EINVAL", s),
            EIO(s) => ("EIO", s),
            EMFILE(s) => ("EMFILE", s),
            ENOENT(s) => ("ENOENT", s),
            ENOSYS(s) => ("ENOSYS", s),
            EPERM(s) => ("EPERM", s),
            Other { errno, syscall } => {
                return write!(f, "{} (errno {})", syscall, errno);
            }
        };
        write!(f, "{} ({})", name, syscall)
    }
}

impl core::error::Error for SyscallError {}
