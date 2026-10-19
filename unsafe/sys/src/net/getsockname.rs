//! `getsockname` — report the local endpoint a socket carries, so a caller
//! that bound `--port 0` can read the kernel-assigned port back (LESSONS,
//! task #239: a pid-derived test port is a shared name; port 0 is no name).

use alloc::vec::Vec;

use crate::{LocalFd, SyscallError, cvt};

/// The local endpoint of a socket, as the kernel stored it at `bind`.
#[derive(Debug)]
pub enum SockName {
    /// AF_INET v4 port in host byte order.
    Inet { port: u16 },
    /// AF_UNIX name bytes: an abstract name without its leading NUL (it is
    /// namespace, not name), a path truncated at its terminating NUL.
    Unix { name: Vec<u8> },
}

/// Report the address `fd` is bound to. A non-socket fd is `ENOTSOCK`, an
/// address family the kernel reports beyond AF_UNIX/AF_INET v4 is `EINVAL`.
pub fn getsockname(fd: &LocalFd) -> Result<SockName, SyscallError> {
    // SAFETY: `sockaddr_storage` is a plain-old-data struct sized for any
    // family's address; the all-zero value is valid (as in `net/addr.rs`).
    let mut storage = unsafe { core::mem::zeroed::<libc::sockaddr_storage>() };
    let mut len = core::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    // SAFETY: `fd` is an open socket; `getsockname` writes the bound address
    // into `storage`, at most the `len` bytes it is given, and the length it
    // wrote into the `socklen_t` it is given. Invalid fds return an errno
    // (`EBADF`/`ENOTSOCK`), caught by `cvt`.
    cvt(unsafe {
        libc::getsockname(
            fd.as_raw(),
            core::ptr::from_mut(&mut storage).cast(),
            core::ptr::from_mut(&mut len).cast(),
        ) as isize
    })?;
    match i32::from(storage.ss_family) {
        libc::AF_INET => {
            // SAFETY: `ss_family` is `AF_INET`, so the kernel wrote a
            // `sockaddr_in` into the storage.
            let sa = unsafe { &*core::ptr::from_ref(&storage).cast::<libc::sockaddr_in>() };
            // `sin_port` is stored network order — the `to_be` note in
            // `net/addr.rs`; `from_be` reports it in host order.
            Ok(SockName::Inet {
                port: u16::from_be(sa.sin_port),
            })
        }
        libc::AF_UNIX => {
            // SAFETY: `ss_family` is `AF_UNIX`, so the kernel wrote a
            // `sockaddr_un` into the storage.
            let sa = unsafe { &*core::ptr::from_ref(&storage).cast::<libc::sockaddr_un>() };
            // The kernel writes back the length of the family's own address;
            // the name is what fills `sun_path` within it (LESSONS: abstract
            // names are length-significant, `net/addr.rs`).
            let off = core::mem::size_of::<libc::sa_family_t>();
            let n = (len as usize).saturating_sub(off).min(sa.sun_path.len());
            let bytes = sa.sun_path.get(..n).ok_or(SyscallError::Never)?;
            let name = if bytes.first().is_some_and(|&c| c == 0) {
                // Abstract: drop the leading NUL.
                bytes.get(1..).ok_or(SyscallError::Never)?
            } else {
                // Path: truncate at the terminating NUL.
                let end = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
                bytes.get(..end).ok_or(SyscallError::Never)?
            };
            Ok(SockName::Unix {
                // `sun_path` holds opaque bytes; `c_char` is `i8` there.
                name: name.iter().map(|&c| c as u8).collect(),
            })
        }
        _ => Err(SyscallError::EINVAL("getsockname")),
    }
}
