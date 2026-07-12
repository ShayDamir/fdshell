//! Conversion from raw errno — default syscall name is "unknown".

use super::SyscallError;
use SyscallError::*;

// Default syscall name when converted from raw errno without context.
const UNKNOWN: &str = "unknown";

impl From<i32> for SyscallError {
    fn from(raw: i32) -> Self {
        match raw {
            libc::E2BIG => E2BIG(UNKNOWN),
            libc::EAGAIN => EAGAIN(UNKNOWN),
            libc::EBADF => EBADF(UNKNOWN),
            libc::EEXIST => EEXIST(UNKNOWN),
            libc::EINVAL => EINVAL(UNKNOWN),
            libc::EIO => EIO(UNKNOWN),
            libc::EMFILE => EMFILE(UNKNOWN),
            libc::ENOENT => ENOENT(UNKNOWN),
            libc::ENOSYS => ENOSYS(UNKNOWN),
            libc::EPERM => EPERM(UNKNOWN),
            n => Other {
                errno: n,
                syscall: UNKNOWN,
            },
        }
    }
}
