//! The standard (glibc `strerror`) descriptions for the errno table — the
//! words the user sees where bash prints them. The errnos the `SyscallError`
//! enum names, plus the common `open`-failure errnos; everything else,
//! including `HELP` (errno 0), is `None` so the caller's fallback arm stays
//! reachable.

use crate::errno::{
    E2BIG, EACCES, EAGAIN, EBADF, EEXIST, EINVAL, EIO, EISDIR, ELOOP, EMFILE, ENAMETOOLONG, ENOENT,
    ENOSYS, ENOTDIR, EPERM,
};

/// The standard description of `errno`, when this table knows it.
pub fn standard(errno: i32) -> Option<&'static str> {
    match errno {
        E2BIG => Some("Argument list too long"),
        EACCES => Some("Permission denied"),
        EAGAIN => Some("Resource temporarily unavailable"),
        EBADF => Some("Bad file descriptor"),
        EEXIST => Some("File exists"),
        EINVAL => Some("Invalid argument"),
        EIO => Some("Input/output error"),
        EISDIR => Some("Is a directory"),
        ELOOP => Some("Too many levels of symbolic links"),
        EMFILE => Some("Too many open files"),
        ENAMETOOLONG => Some("File name too long"),
        ENOENT => Some("No such file or directory"),
        ENOSYS => Some("Function not implemented"),
        ENOTDIR => Some("Not a directory"),
        EPERM => Some("Operation not permitted"),
        _ => None,
    }
}
