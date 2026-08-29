use crate::SyscallError;

pub const R_OK: i32 = libc::R_OK;
pub const W_OK: i32 = libc::W_OK;
pub const X_OK: i32 = libc::X_OK;

/// `access(2)`: whether the calling process may `mode`-access `path`.
pub fn access(path: &core::ffi::CStr, mode: i32) -> Result<(), SyscallError> {
    // SAFETY: `path` is a valid null-terminated C string; a bad path or mode
    // returns -1 (ENOENT/EINVAL), caught by `cvt`.
    crate::cvt(unsafe { libc::access(path.as_ptr(), mode) as isize }).map(|_| ())
}
