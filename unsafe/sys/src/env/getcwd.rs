//! `getcwd(3)` with a growing buffer: a CWD path longer than 4 KiB (reachable
//! via relative `mkdir` chains beyond `PATH_MAX`) fails the fixed buffer.

use alloc::vec::Vec;
use core::ffi::CStr;

use crate::SyscallError;

/// First attempt: a stack buffer, so the common path allocates nothing.
const INITIAL_GETCWD_SIZE: usize = 4096;
/// Largest buffer the retry loop will allocate (1 MiB); a longer CWD returns
/// the last too-small-buffer error instead of allocating more.
const MAX_GETCWD_SIZE: usize = 1 << 20;

/// Return the current working directory.
///
/// Tries a 4 KiB stack buffer first; on a too-small-buffer errno (`ERANGE` on
/// modern kernels, `ENAMETOOLONG` per `getcwd(2)`) retries with a heap buffer
/// that doubles in size up to `MAX_GETCWD_SIZE`.
pub fn getcwd() -> Result<Vec<u8>, SyscallError> {
    let mut buf = [0u8; INITIAL_GETCWD_SIZE];
    match getcwd_into(&mut buf) {
        Ok(len) => return Ok(buf.get(..len).ok_or(SyscallError::Never)?.to_vec()),
        Err(e) if buffer_too_small(e) => {}
        Err(e) => return Err(e),
    }
    let mut size = INITIAL_GETCWD_SIZE;
    let mut err = SyscallError::Other {
        errno: libc::ERANGE,
        syscall: "getcwd",
    };
    loop {
        let doubled = size.saturating_mul(2);
        if doubled > MAX_GETCWD_SIZE {
            return Err(err);
        }
        size = doubled;
        let mut buf = alloc::vec![0u8; size];
        match getcwd_into(&mut buf) {
            Ok(len) => {
                buf.truncate(len);
                return Ok(buf);
            }
            Err(e) if buffer_too_small(e) => err = e,
            // Defensive arm: a non-too-small errno (e.g. the CWD deleted
            // between the initial and a heap attempt) is returned as-is.
            // Only reachable via a race, so no test covers it.
            Err(e) => return Err(e),
        }
    }
}

/// `ERANGE` (modern kernels) or `ENAMETOOLONG` (`getcwd(2)`): the buffer is
/// too small for the absolute CWD path. The `ENAMETOOLONG` disjunct is a
/// portability branch per `getcwd(2)` — this platform's `getcwd(3)` reports
/// `ERANGE`, so no test makes it match (other errnos, e.g. `ENOENT` from a
/// deleted CWD, only probe it).
fn buffer_too_small(e: SyscallError) -> bool {
    e.errno() == libc::ERANGE || e.errno() == libc::ENAMETOOLONG
}

/// One `getcwd(3)` call into `buf`; returns the path length written.
fn getcwd_into(buf: &mut [u8]) -> Result<usize, SyscallError> {
    // SAFETY: `buf` is a valid buffer; `getcwd` writes at most `buf.len()`
    // bytes including the NUL terminator, or returns NULL and sets errno.
    let ret = unsafe { libc::getcwd(buf.as_mut_ptr().cast(), buf.len()) };
    if ret.is_null() {
        // SAFETY: `__errno_location` returns a valid pointer to thread-local
        // errno, read immediately after the failed call.
        let errno = unsafe { *libc::__errno_location() };
        return Err(SyscallError::Other {
            errno,
            syscall: "getcwd",
        });
    }
    // SAFETY: glibc returns a pointer into `buf` (never an allocation), which
    // holds a NUL-terminated path.
    Ok(unsafe { CStr::from_ptr(ret).to_bytes().len() })
}
