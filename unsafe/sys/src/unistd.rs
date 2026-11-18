// mutants: -C --test -C unistd -- -E 'binary(unistd)'

//! `<unistd.h>` wrappers — the process-identity queries the prompt escapes
//! need: `geteuid` for `\$`, `gethostname` for `\h`/`\H`, `ttyname` for `\l`.
//! Strings come back as `Vec<u8>` — the sanctioned imported-string shape
//! (STYLE §6.2); the safe crate converts them at its own edge (§6.3).

use alloc::vec::Vec;
use core::ffi::CStr;

use crate::{ImportedFd, LocalFd, SyscallError};

/// Bigger than the kernel's UTS node-name limit, so the name always fits and
/// the answer is always NUL-terminated: no truncation branch.
const HOSTNAME_BUF: usize = 256;

/// `geteuid(2)` — the effective uid. Infallible on Linux (glibc caches it),
/// like `env::getpid()`.
pub fn geteuid() -> u32 {
    // SAFETY: `geteuid()` always succeeds; it takes no arguments and touches
    // no memory.
    unsafe { libc::geteuid() }
}

/// `gethostname(2)` — the UTS node name, NUL-trimmed.
pub fn gethostname() -> Result<Vec<u8>, SyscallError> {
    let mut buf = [0u8; HOSTNAME_BUF];
    // SAFETY: `buf` is writable memory of `buf.len()` bytes and the name fits
    // (the kernel limit is smaller than the buffer).
    crate::cvt(unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } as isize)?;
    // SAFETY: the buffer is zero-initialized and the name fits, so its bytes
    // form a NUL-terminated C string.
    Ok(unsafe { CStr::from_ptr(buf.as_ptr().cast()) }
        .to_bytes()
        .to_vec())
}

impl LocalFd {
    /// `ttyname(3)` — the terminal path this fd names, NUL-trimmed; `Err`
    /// (errno) when the fd is not a terminal.
    pub fn ttyname(&self) -> Result<Vec<u8>, SyscallError> {
        ttyname_raw(self.as_raw())
    }
}

impl ImportedFd {
    /// `ttyname(3)` — the terminal path this fd names, NUL-trimmed; `Err`
    /// (errno) when the fd is not a terminal.
    pub fn ttyname(&self) -> Result<Vec<u8>, SyscallError> {
        ttyname_raw(self.as_raw())
    }
}

/// The one `<unistd.h>` call behind both fd-type impls (raw fds are legal
/// inside this crate, STYLE §5.1). The glibc-static buffer is copied out
/// immediately — the next call may reuse it.
fn ttyname_raw(fd: i32) -> Result<Vec<u8>, SyscallError> {
    // SAFETY: `fd` is a valid open fd by the caller's type invariant;
    // `ttyname` returns NULL and sets errno for a non-terminal fd, else a
    // pointer into a glibc-static buffer.
    let ptr = unsafe { libc::ttyname(fd) };
    if ptr.is_null() {
        // SAFETY: `__errno_location` returns a valid pointer to thread-local
        // errno, read immediately after the failed call.
        return Err(unsafe { *libc::__errno_location() }.into());
    }
    // SAFETY: `ptr` is non-null, so it points to a NUL-terminated C string
    // owned by glibc; copied before any further libc call.
    Ok(unsafe { CStr::from_ptr(ptr) }.to_bytes().to_vec())
}
