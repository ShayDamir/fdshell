//! `bind` — bind a socket to an AF_UNIX (path or abstract) or AF_INET v4
//! address.

use core::ffi::CStr;

use crate::{LocalFd, cvt};

use super::addr::{inet_addr, uds_addr};
use crate::SyscallError;

/// Bind `fd` to an AF_UNIX filesystem socket at `path` (the kernel resolves
/// it against the caller's CWD). A socket file is created at `path`.
pub fn bind_uds_path<P: AsRef<CStr>>(fd: &LocalFd, path: P) -> Result<(), SyscallError> {
    let (addr, len) = uds_addr(path.as_ref(), false, "bind")?;
    bind_addr(fd, &addr, len)
}

/// Bind `fd` to an AF_UNIX abstract-namespace address: no filesystem object
/// is created, so there is no path TOCTOU and no stale socket file to clean
/// up.
pub fn bind_uds_abstract<P: AsRef<CStr>>(fd: &LocalFd, name: P) -> Result<(), SyscallError> {
    let (addr, len) = uds_addr(name.as_ref(), true, "bind")?;
    bind_addr(fd, &addr, len)
}

/// Bind `fd` to an AF_INET v4 endpoint: dotted-quad `addr` and `port`
/// (`inet_pton` rejects non-numeric input with `EINVAL`).
pub fn bind_inet<P: AsRef<CStr>>(fd: &LocalFd, addr: P, port: u16) -> Result<(), SyscallError> {
    let sa = inet_addr(addr.as_ref(), port)?;
    bind_addr(fd, &sa, core::mem::size_of::<libc::sockaddr_in>())
}

/// Bind `fd` to the address structure `addr` (`len` = its size).
fn bind_addr<T>(fd: &LocalFd, addr: &T, len: usize) -> Result<(), SyscallError> {
    let ptr = core::ptr::from_ref(addr).cast();
    // SAFETY: `addr` is a valid address structure of `len` bytes; `fd` is an
    // open socket; `bind` reads at most `len` bytes from it.
    cvt(unsafe { libc::bind(fd.as_raw(), ptr, len as _) as isize })?;
    Ok(())
}
