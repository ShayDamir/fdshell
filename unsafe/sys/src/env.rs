//! Environment and process query wrappers — safe interfaces over libc.
//!
//! All functions use `libc` directly; no `std` dependency.

#![allow(clippy::indexing_slicing)]

use alloc::vec::Vec;
use core::ffi::CStr;

use crate::SyscallError;

/// Return the current process ID.
pub fn getpid() -> i32 {
    // SAFETY: `getpid()` always succeeds and returns a valid PID.
    unsafe { libc::getpid() }
}

/// Look up an environment variable by name.
///
/// Returns the value as an owned `Vec<u8>` (without trailing NUL), or `None`
/// if the variable is not set.
pub fn getenv(name: &[u8]) -> Option<Vec<u8>> {
    // SAFETY: `name` is a valid byte slice; we append NUL for the C string.
    let key = alloc::ffi::CString::new(name).ok()?;
    // SAFETY: `getenv` returns a pointer to the process environment; the
    // caller copies the value so no lifetime issues arise.
    let ptr = unsafe { libc::getenv(key.as_ptr()) };
    if ptr.is_null() {
        return None;
    }
    // SAFETY: `ptr` points to a NUL-terminated C string (the environment).
    let cstr = unsafe { CStr::from_ptr(ptr) };
    let bytes: Vec<u8> = cstr.to_bytes().to_vec();
    Some(bytes)
}

/// Return the current working directory.
///
/// Uses a 4096-byte buffer (sufficient for PATH_MAX on Linux).
pub fn getcwd() -> Result<Vec<u8>, SyscallError> {
    let mut buf = [0u8; 4096];
    // SAFETY: `buf` is a valid, sufficiently-sized buffer; `getcwd` writes at
    // most PATH_MAX bytes and NUL-terminates. On success the returned pointer
    // equals `buf`; on failure it returns NULL and sets errno.
    let ret = unsafe { libc::getcwd(buf.as_mut_ptr().cast(), buf.len()) };
    if ret.is_null() {
        // SAFETY: __errno_location returns a valid pointer to thread-local errno.
        let errno = unsafe { *libc::__errno_location() };
        return Err(crate::SyscallError::Other {
            errno,
            syscall: "getcwd",
        });
    }
    // SAFETY: `ret` points into `buf`, which is valid memory.
    let len = unsafe { CStr::from_ptr(ret).to_bytes().len() };
    Ok(buf[..len].to_vec()) // SAFETY: len is the length of the CStr from buf, so it's always ≤ buf.len().
}

/// Read `/proc/self/cmdline` and split on NUL bytes.
///
/// Returns each argument as a `Vec<u8>`. Empty input yields an empty `Vec`.
pub fn read_cmdline() -> Result<Vec<Vec<u8>>, SyscallError> {
    use crate::fcntl::O_RDONLY;

    let fd = crate::openat2::open(c"/proc/self/cmdline", O_RDONLY)?;
    let mut buf = [0u8; 4096];
    let n = crate::rw::read(&fd, &mut buf)? as usize;
    // SAFETY: fd is about to be closed; read already succeeded.
    unsafe { libc::close(fd.as_raw()) };
    if n == 0 {
        return Ok(Vec::new());
    }
    // Split on NUL bytes. Only discard a single trailing empty fragment
    // (the artifact of the final NUL terminator); preserve all other
    // arguments including empty ones.
    let mut parts: Vec<Vec<u8>> = buf[..n] // SAFETY: n is the number of bytes read, which is ≤ buf.len().
        .split(|&b| b == b'\0')
        .map(|s| s.to_vec())
        .collect();
    if parts.last().is_some_and(|s| s.is_empty()) {
        parts.pop();
    }
    Ok(parts)
}
/// Parse the C `environ` array into a `Vec` of `(key, value)` pairs.
///
/// Skips entries without an `=` sign (same as `std::env::vars()`).
pub fn environ_snapshot() -> Vec<(Vec<u8>, Vec<u8>)> {
    // SAFETY: `environ` is a NULL-terminated array of C strings provided by the C runtime.
    let environ_ptr = unsafe { environ };
    let mut result = Vec::new();
    if environ_ptr.is_null() {
        return result;
    }
    let mut envp = environ_ptr;
    loop {
        // SAFETY: `environ` is a NULL-terminated array; we stop at NULL.
        let entry = unsafe { *envp };
        if entry.is_null() {
            break;
        }
        // SAFETY: `entry` points to a NUL-terminated C string from the environment.
        let cstr = unsafe { CStr::from_ptr(entry) };
        let bytes = cstr.to_bytes();
        if let Some(eq_pos) = bytes.iter().position(|&b| b == b'=') {
            let key = bytes[..eq_pos].to_vec(); // SAFETY: eq_pos < bytes.len() because iter::position found it.
            let value = bytes[eq_pos + 1..].to_vec(); // SAFETY: eq_pos + 1 ≤ bytes.len() because position found '=' in bytes.
            result.push((key, value));
        }
        // SAFETY: environ is a NULL-terminated array; we check for NULL above.
        envp = unsafe { envp.add(1) };
    }
    result
}

// Module-level extern block for environ (accessible from any fn in this module).
unsafe extern "C" {
    static environ: *const *const libc::c_char;
}
