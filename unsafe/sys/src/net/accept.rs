// mutants: -C --test -C sockets -- -E 'binary(sockets)'
//! `accept` — dequeue the next connection on a listening socket.

use crate::{LocalFd, cvt};

impl LocalFd {
    /// Accept the next connection on the listening socket `self`, returning a new
    /// `CLOEXEC` fd for the connected peer (`accept4` + `SOCK_CLOEXEC`, no peer
    /// address read-back). Blocks until a connection arrives or `self` errors;
    /// a non-listening or dgram socket fails with `EINVAL`/`EOPNOTSUPP`.
    pub fn accept(&self) -> Result<LocalFd, crate::SyscallError> {
        // SAFETY: `self` is a valid open socket; null peer-address pointers skip
        // the read-back; `accept4` returns a new fd on success or -1, checked
        // by `cvt`.
        let ret = cvt(unsafe {
            libc::accept4(
                self.as_raw(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                libc::SOCK_CLOEXEC,
            ) as isize
        })?;
        // SAFETY: `SOCK_CLOEXEC` is set, satisfying the `LocalFd` invariant.
        Ok(unsafe { LocalFd::from_raw(ret as i32) })
    }
}
