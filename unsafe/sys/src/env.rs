//! Environment and process query wrappers — safe interfaces over libc.
//!
//! All functions use `libc` directly; no `std` dependency.

use alloc::vec::Vec;
use core::ffi::CStr;

use error_stack::{Report, ResultExt, ensure};

use crate::SyscallError;
use crate::shortcstr::ShortCStr;

/// Error type for `read_cmdline`.
#[derive(Debug, displaydoc::Display)]
pub enum ReadCmdlineError {
    /// failed to open /proc/self/cmdline
    OpenFailed,
    /// argument contains a NUL byte
    InvalidArg,
    /// command line is empty (missing argv[0])
    EmptyCmdline,
    /// impossible state (indexing invariant violation)
    Never,
}

impl core::error::Error for ReadCmdlineError {}

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
    Ok(buf.get(..len).ok_or(SyscallError::Never)?.to_vec())
}

/// Read `/proc/self/cmdline` and split on NUL bytes.
///
/// Returns each argument as a `ShortCStr`. The command line must contain at least `argv[0]`.
pub fn read_cmdline() -> Result<Vec<ShortCStr>, Report<ReadCmdlineError>> {
    use crate::fcntl::O_RDONLY;

    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let fd = crate::openat2::open(c"/proc/self/cmdline", O_RDONLY)
        .change_context(ReadCmdlineError::OpenFailed)?;
    loop {
        let n =
            crate::rw::read(&fd, &mut chunk).change_context(ReadCmdlineError::OpenFailed)? as usize;
        if n == 0 {
            break;
        }
        let slice = chunk.get(..n).ok_or(ReadCmdlineError::Never)?;
        buf.extend_from_slice(slice);
    }
    ensure!(!buf.is_empty(), ReadCmdlineError::EmptyCmdline);
    let mut parts: Vec<ShortCStr> = buf
        .split(|&b| b == b'\0')
        .map(|f| ShortCStr::from_vec(f.to_vec()))
        .collect::<Result<Vec<_>, _>>()
        .change_context(ReadCmdlineError::InvalidArg)?;
    if parts.last().is_some_and(|p| p.is_empty()) {
        let _ = parts.pop();
    }
    Ok(parts)
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
