//! `bind` — bind a socket to an AF_UNIX (path or abstract) or AF_INET v4
//! address.

use core::ffi::CStr;

use crate::{LocalFd, cvt};

use crate::SyscallError;

// `libc` 0.2.186 dropped the `inet_pton` binding; glibc always exports it.
// SAFETY: `inet_pton` reads a valid NUL-terminated string and writes at most
// 4 bytes to `buf`; no other state is touched.
unsafe extern "C" {
    fn inet_pton(af: libc::c_int, cp: *const libc::c_char, buf: *mut libc::c_void) -> libc::c_int;
}

/// Longest address that fits `sun_path` (108 bytes): the path form needs room
/// for the terminating NUL, the abstract form for its leading NUL.
const SUN_PATH_MAX: usize = 107;

/// Bind `fd` to an AF_UNIX filesystem socket at `path` (the kernel resolves
/// it against the caller's CWD). A socket file is created at `path`.
pub fn bind_uds_path<P: AsRef<CStr>>(fd: &LocalFd, path: P) -> Result<(), SyscallError> {
    let path = path.as_ref();
    let addr = uds_addr(path, false)?;
    bind_addr(fd, &addr, core::mem::size_of::<libc::sockaddr_un>())
}

/// Bind `fd` to an AF_UNIX abstract-namespace address: no filesystem object
/// is created, so there is no path TOCTOU and no stale socket file to clean
/// up.
pub fn bind_uds_abstract<P: AsRef<CStr>>(fd: &LocalFd, name: P) -> Result<(), SyscallError> {
    let name = name.as_ref();
    let addr = uds_addr(name, true)?;
    // An abstract name is not NUL-terminated: the kernel takes its length
    // from `addrlen`, so pass exactly family + leading NUL + name bytes. A
    // padded length folds the zero tail into the address and peers connecting
    // with the true name get ECONNREFUSED (probed).
    let len = core::mem::size_of::<libc::sa_family_t>() + 1 + name.to_bytes().len();
    bind_addr(fd, &addr, len)
}

/// Bind `fd` to an AF_INET v4 endpoint: dotted-quad `addr` and `port`
/// (`inet_pton` rejects non-numeric input with `EINVAL`).
pub fn bind_inet<P: AsRef<CStr>>(fd: &LocalFd, addr: P, port: u16) -> Result<(), SyscallError> {
    let addr = addr.as_ref();
    let mut addr4 = [0u8; 4];
    // SAFETY: `addr4` is valid 4-byte `in_addr` storage; `addr` is a valid
    // NUL-terminated string; `inet_pton` writes at most 4 bytes.
    let ok = unsafe {
        inet_pton(
            libc::AF_INET,
            addr.as_ptr(),
            core::ptr::from_mut(&mut addr4).cast(),
        )
    };
    if ok != 1 {
        return Err(SyscallError::EINVAL("inet_pton"));
    }
    // `inet_pton` leaves `addr4` in network byte order; `from_ne_bytes`
    // keeps those exact bytes in memory on either endianness, which is how
    // the kernel reads `s_addr` (probed: `from_be_bytes` byte-swapped the
    // address on little-endian and bound the wrong endpoint).
    let sa = libc::sockaddr_in {
        sin_family: libc::AF_INET as _,
        sin_port: port.to_be(),
        sin_addr: libc::in_addr {
            s_addr: u32::from_ne_bytes(addr4),
        },
        sin_zero: [0; 8],
    };
    bind_addr(fd, &sa, core::mem::size_of::<libc::sockaddr_in>())
}

/// Fill a `sockaddr_un` from `name`: abstract addresses carry a leading NUL
/// byte, path addresses a terminating one. Both leave room for it in the
/// 108-byte `sun_path`, so `name` must be at most `SUN_PATH_MAX` bytes.
fn uds_addr(name: &CStr, abstract_: bool) -> Result<libc::sockaddr_un, SyscallError> {
    let bytes = name.to_bytes();
    if bytes.len() > SUN_PATH_MAX {
        return Err(SyscallError::EINVAL("bind"));
    }
    // SAFETY: `sockaddr_un` is a plain-old-data struct (a 16-bit family plus
    // a byte array); the all-zero value is valid.
    let mut sa = unsafe { core::mem::zeroed::<libc::sockaddr_un>() };
    sa.sun_family = libc::AF_UNIX as u16;
    // The buffer is zeroed, so the NUL (leading or trailing) is already in
    // place; only the name bytes are copied. `sun_path` is `[i8; 108]` but
    // holds opaque bytes.
    let dst = sa
        .sun_path
        .get_mut(if abstract_ { 1 } else { 0 }..)
        .ok_or(SyscallError::Never)?
        .get_mut(..bytes.len())
        .ok_or(SyscallError::Never)?;
    // SAFETY: both regions are valid for `bytes.len()` bytes, do not alias,
    // and are properly aligned for `u8`.
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), dst.as_mut_ptr().cast::<u8>(), bytes.len())
    };
    Ok(sa)
}

/// Bind `fd` to the address structure `addr` (`len` = its size).
fn bind_addr<T>(fd: &LocalFd, addr: &T, len: usize) -> Result<(), SyscallError> {
    let ptr = core::ptr::from_ref(addr).cast();
    // SAFETY: `addr` is a valid address structure of `len` bytes; `fd` is an
    // open socket; `bind` reads at most `len` bytes from it.
    cvt(unsafe { libc::bind(fd.as_raw(), ptr, len as _) as isize })?;
    Ok(())
}
