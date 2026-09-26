//! Environment and process query wrappers — safe interfaces over libc.
//!
//! All functions use `libc` directly; no `std` dependency.

use alloc::vec::Vec;
use core::ffi::CStr;

use crate::pid::Pid;
use crate::shortcstr::ShortCStr;

mod getcwd;

pub use getcwd::getcwd;

/// Return the current process ID.
pub fn getpid() -> Pid {
    // SAFETY: `getpid()` always succeeds and returns a valid PID.
    Pid::from_raw(unsafe { libc::getpid() })
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
