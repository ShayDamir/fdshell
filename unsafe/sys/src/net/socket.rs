//! `socket` — create a socket with `CLOEXEC` always set.

use crate::LocalFd;

/// Create a socket of type `ty` (`SOCK_STREAM`/`SOCK_DGRAM`) in `domain`
/// (`AF_UNIX`/`AF_INET`), protocol 0, with `CLOEXEC` always set.
pub fn socket(domain: i32, ty: i32) -> Result<LocalFd, crate::SyscallError> {
    // SAFETY: `socket` returns a new fd on success or -1 on error, checked
    // by `cvt`; `domain` and `ty` are valid constants.
    let ret = crate::cvt(unsafe { libc::socket(domain, ty + libc::SOCK_CLOEXEC, 0) as isize })?;
    // SAFETY: `SOCK_CLOEXEC` is set, satisfying the `LocalFd` invariant.
    Ok(unsafe { LocalFd::from_raw(ret as i32) })
}
