//! `accept` — dequeue the next connection on a listening socket.

use crate::{LocalFd, cvt};

/// Accept the next connection on listening socket `fd`, returning a new
/// `CLOEXEC` fd for the connected peer (`accept4` + `SOCK_CLOEXEC`, no peer
/// address read-back). Blocks until a connection arrives or `fd` errors;
/// a non-listening or dgram socket fails with `EINVAL`/`EOPNOTSUPP`.
pub fn accept(fd: &LocalFd) -> Result<LocalFd, crate::SyscallError> {
    // SAFETY: `fd` is a valid open socket; null peer-address pointers skip
    // the read-back; `accept4` returns a new fd on success or -1, checked
    // by `cvt`.
    let ret = cvt(unsafe {
        libc::accept4(
            fd.as_raw(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            libc::SOCK_CLOEXEC,
        ) as isize
    })?;
    // SAFETY: `SOCK_CLOEXEC` is set, satisfying the `LocalFd` invariant.
    Ok(unsafe { LocalFd::from_raw(ret as i32) })
}
