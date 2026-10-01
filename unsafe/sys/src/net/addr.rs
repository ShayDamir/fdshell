//! Shared AF_UNIX/AF_INET `sockaddr` construction for the `bind` and
//! `connect` wrappers: the exact-`addrlen` rule (LESSONS) lives here once.

use core::ffi::CStr;

use crate::SyscallError;

// `libc` 0.2.186 dropped the `inet_pton` binding; glibc always exports it.
// SAFETY: `inet_pton` reads a valid NUL-terminated string and writes at most
// 4 bytes to `buf`; no other state is touched.
unsafe extern "C" {
    fn inet_pton(af: libc::c_int, cp: *const libc::c_char, buf: *mut libc::c_void) -> libc::c_int;
}

/// Longest address that fits `sun_path` (108 bytes): the path form needs room
/// for the terminating NUL, the abstract form for its leading NUL.
pub(crate) const SUN_PATH_MAX: usize = 107;

/// Fill a `sockaddr_un` from `name` and return the `addrlen` to pass to the
/// syscall: abstract addresses carry a leading NUL byte, path addresses a
/// terminating one. Both leave room for it in the 108-byte `sun_path`, so
/// `name` must be at most `SUN_PATH_MAX` bytes.
///
/// The path length is the whole `sockaddr_un` (the kernel stops at the
/// terminating NUL); an abstract name is not NUL-terminated, so its length is
/// exactly family + leading NUL + name bytes — a padded length folds the zero
/// tail into the address (LESSONS). `call` names the syscall in the over-long
/// `EINVAL` so each wrapper reports its own name.
pub(crate) fn uds_addr(
    name: &CStr,
    abstract_: bool,
    call: &'static str,
) -> Result<(libc::sockaddr_un, usize), SyscallError> {
    let bytes = name.to_bytes();
    if bytes.len() > SUN_PATH_MAX {
        return Err(SyscallError::EINVAL(call));
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
    let len = if abstract_ {
        core::mem::size_of::<libc::sa_family_t>() + 1 + bytes.len()
    } else {
        core::mem::size_of::<libc::sockaddr_un>()
    };
    Ok((sa, len))
}

/// Build a `sockaddr_in` from dotted-quad `addr` and `port` (`inet_pton`
/// rejects non-numeric input with `EINVAL("inet_pton")`).
pub(crate) fn inet_addr(addr: &CStr, port: u16) -> Result<libc::sockaddr_in, SyscallError> {
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
    Ok(libc::sockaddr_in {
        sin_family: libc::AF_INET as _,
        sin_port: port.to_be(),
        sin_addr: libc::in_addr {
            s_addr: u32::from_ne_bytes(addr4),
        },
        sin_zero: [0; 8],
    })
}
