//! Environment and process query wrappers — safe interfaces over libc.
//!
//! All functions use `libc` directly; no `std` dependency.

use alloc::vec::Vec;
use core::ffi::CStr;

use crate::SyscallError;
use crate::shortcstr::ShortCStr;

/// Return the current process ID.
pub fn getpid() -> i32 {
    // SAFETY: `getpid()` always succeeds and returns a valid PID.
    unsafe { libc::getpid() }
}

/// Look up an environment variable by name.
///
/// Returns the value as an owned `ShortCStr`, or `None` if the variable is not set.
pub fn getenv(name: &CStr) -> Option<ShortCStr> {
    // SAFETY: `getenv` returns a pointer to the process environment; the
    // caller copies the value so no lifetime issues arise.
    let ptr = unsafe { libc::getenv(name.as_ptr()) };
    if ptr.is_null() {
        return None;
    }
    // SAFETY: `ptr` points to a NUL-terminated C string (the environment).
    let cstr = unsafe { CStr::from_ptr(ptr) };
    ShortCStr::from_vec(cstr.to_bytes().to_vec()).ok()
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
    Ok(buf.get(..len).ok_or(SyscallError::Never)?.to_vec())
}

/// Parse the C `environ` array into a `Vec` of `(key, value)` pairs.
///
/// Skips entries without an `=` sign (same as `std::env::vars()`).
pub fn environ_snapshot() -> Vec<(ShortCStr, ShortCStr)> {
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
        // SAFETY: `entry` points to a NUL-terminated C string from the environment;
        // environ strings live for the duration of the program.
        let cstr = unsafe { CStr::from_ptr(entry) };
        let short = ShortCStr::from(cstr);
        if let Some((key, value)) = short.split_once_byte(b'=') {
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
