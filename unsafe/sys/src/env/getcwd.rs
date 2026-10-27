//! `getcwd(3)` with a growing buffer: a CWD path longer than 4 KiB (reachable
//! via relative `mkdir` chains beyond `PATH_MAX`) fails the fixed buffer.

use alloc::vec::Vec;
use core::ffi::CStr;

use crate::SyscallError;

/// First attempt: a stack buffer, so the common path allocates nothing.
const INITIAL_GETCWD_SIZE: usize = 4096;
/// Largest buffer the retry loop will allocate (1 MiB). Past it the answer is
/// `E2BIG("getcwd")` — **no buffer we will allocate fits** — not "the last
/// errno the kernel returned". Naming the exhaustion state is what makes the
/// too-small guard observable: while the loop's answer was the last errno it
/// saw, a guard mutant that swallowed a foreign errno and retried in unchanged
/// process state returned a byte-identical error, and only the syscall count
/// differed, so no test could tell the retry from the bail.
const MAX_GETCWD_SIZE: usize = 1 << 20;

/// Return the current working directory.
///
/// Tries a 4 KiB stack buffer first; on a too-small-buffer errno (`ERANGE` on
/// modern kernels, `ENAMETOOLONG` per `getcwd(2)`) retries with a heap buffer
/// that doubles in size up to `MAX_GETCWD_SIZE`, and reports `E2BIG` past it.
pub fn getcwd() -> Result<Vec<u8>, SyscallError> {
    let mut stack = [0u8; INITIAL_GETCWD_SIZE];
    let mut heap: Vec<u8> = Vec::new();
    loop {
        // **One** too-small guard, and one buffer chosen by whether the heap
        // has been needed yet: the stack buffer keeps the common path
        // allocation-free. Two arms restating the same guard made the initial
        // arm's copy a rule no test can reach — mutating it merely repeats an
        // attempt whose answer the loop was built to converge on.
        let from_stack = heap.is_empty();
        let len = match if from_stack {
            getcwd_into(&mut stack)
        } else {
            getcwd_into(&mut heap)
        } {
            Ok(len) => len,
            Err(e) if buffer_too_small(e) => {
                let doubled = heap.len().max(INITIAL_GETCWD_SIZE).saturating_mul(2);
                if doubled > MAX_GETCWD_SIZE {
                    return Err(SyscallError::E2BIG("getcwd"));
                }
                heap = alloc::vec![0u8; doubled];
                continue;
            }
            // A non-too-small errno (the CWD deleted, an unreadable ancestor)
            // is the answer as-is: the initial attempt lands here directly, a
            // mid-loop one only via a race.
            Err(e) => return Err(e),
        };
        let path = if from_stack {
            stack.get(..len)
        } else {
            heap.get(..len)
        };
        return Ok(path.ok_or(SyscallError::Never)?.to_vec());
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
