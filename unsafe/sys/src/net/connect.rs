//! `connect` — connect a socket to a peer AF_UNIX (path or abstract) or
//! AF_INET v4 address.

use core::ffi::CStr;

use crate::{LocalFd, cvt};

use super::addr::{inet_addr, uds_addr};
use crate::SyscallError;

/// Connect `fd` to an AF_UNIX filesystem socket at `path` (the kernel
/// resolves it against the caller's CWD). The socket file must already
/// exist and be listening — `connect` never creates one.
pub fn connect_uds_path<P: AsRef<CStr>>(fd: &LocalFd, path: P) -> Result<(), SyscallError> {
    let (addr, len) = uds_addr(path.as_ref(), false, "connect")?;
    connect_addr(fd, &addr, len)
}

/// Connect `fd` to an AF_UNIX abstract-namespace address. No filesystem
/// object is involved; the name must be bound and (for stream sockets)
/// listening, else the kernel refuses the connect (`ECONNREFUSED`).
pub fn connect_uds_abstract<P: AsRef<CStr>>(fd: &LocalFd, name: P) -> Result<(), SyscallError> {
    let (addr, len) = uds_addr(name.as_ref(), true, "connect")?;
    connect_addr(fd, &addr, len)
}

/// Connect `fd` to an AF_INET v4 endpoint: dotted-quad `addr` and `port`
/// (`inet_pton` rejects non-numeric input with `EINVAL`).
pub fn connect_inet<P: AsRef<CStr>>(fd: &LocalFd, addr: P, port: u16) -> Result<(), SyscallError> {
    let sa = inet_addr(addr.as_ref(), port)?;
    connect_addr(fd, &sa, core::mem::size_of::<libc::sockaddr_in>())
}

/// Connect `fd` to the address structure `addr` (`len` = its size).
fn connect_addr<T>(fd: &LocalFd, addr: &T, len: usize) -> Result<(), SyscallError> {
    let ptr = core::ptr::from_ref(addr).cast();
    // SAFETY: `addr` is a valid address structure of `len` bytes; `fd` is an
    // open socket; `connect` reads at most `len` bytes from it.
    cvt(unsafe { libc::connect(fd.as_raw(), ptr, len as _) as isize })?;
    Ok(())
}
