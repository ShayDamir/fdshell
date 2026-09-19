//! `memfd_create(2)` / `memfd_set_seal(2)` anonymous in-memory files.
//!
//! A memfd is an anonymous file that lives in RAM (or swap) under a name with
//! no filesystem entry, so it needs no directory, leaks no path, and vanishes
//! when its last fd closes. It backs temp-file-less heredocs and sealed
//! secrets: a memfd created with `MFD_ALLOW_SEALING` can have `F_SEAL_*` flags
//! applied via `memfd_set_seal` to forbid shrinking, growing, writing, or
//! mapping it, after which even the owner can no longer change it.
//!
//! `memfd_create` uses its `libc` wrapper; `memfd_set_seal` uses the
//! `F_ADD_SEALS` `fcntl(2)` command, which `libc` does not export, so the
//! command constant is defined here and the call goes through `libc::fcntl`.

use core::ffi::CStr;

use crate::{LocalFd, SyscallError, cvt};

/// `memfd_create(2)` flags, from `<linux/memfd.h>`.
pub const MFD_CLOEXEC: u32 = 0x0001;
pub const MFD_ALLOW_SEALING: u32 = 0x0002;

/// `memfd_set_seal(2)` seal masks, from `<linux/fcntl.h>`.
///
/// `libc` does not export these for the GNU linux target, so the syscall
/// wrapper re-exports them and the safe builtins crate uses the re-exports
/// instead of hardcoding flag values.
pub use libc::{
    F_SEAL_EXEC, F_SEAL_FUTURE_WRITE, F_SEAL_GROW, F_SEAL_SEAL, F_SEAL_SHRINK, F_SEAL_WRITE,
};

/// `F_ADD_SEALS` — the `fcntl(2)` command `memfd_set_seal` is built on (from
/// `<linux/fcntl.h>`). `libc` does not export it, so the wrapper defines it
/// here; the legacy `F_SEAL_ADD` ioctl form is not handled on current kernels
/// (it returns ENOTTY), so the fcntl form is the only working interface.
const F_ADD_SEALS: i32 = 1033;

/// Create an anonymous in-memory file with the given name and flags.
///
/// `name` may be `None` for a fully anonymous memfd; a name is visible in
/// `/proc/<pid>/maps` and must be ≤ 14 bytes with no `/` or NUL (validated by
/// the caller). The kernel has no NULL special case — `memfd_create` always
/// copies the name from userspace (`strncpy_from_user`), so a NULL name faults
/// with EFAULT; `None` is passed as the empty name, which shows up in
/// `/proc/<pid>/maps` as `memfd:`. `flags` must include `MFD_CLOEXEC` (for a
/// [`LocalFd`]) and, to allow sealing, `MFD_ALLOW_SEALING`.
pub fn memfd_create_with_name_and_flags(
    name: Option<&CStr>,
    flags: u32,
) -> Result<LocalFd, SyscallError> {
    let ptr = match name {
        Some(c) => c.as_ptr(),
        None => c"".as_ptr(),
    };
    // SAFETY: `ptr` is always a valid NUL-terminated userspace string (the
    // empty name for anonymous — never NULL) that `memfd_create` reads; it
    // writes one fd or -1, checked by `cvt`; `MFD_CLOEXEC` is the caller's
    // responsibility and is satisfied by the callers.
    let ret = cvt(unsafe { libc::memfd_create(ptr, flags) as isize })?;
    // SAFETY: `MFD_CLOEXEC` is set by the callers, satisfying the `LocalFd`
    // invariant.
    Ok(unsafe { LocalFd::from_raw(ret as i32) })
}

/// Create a fully anonymous `CLOEXEC` memfd.
pub fn memfd_create() -> Result<LocalFd, SyscallError> {
    memfd_create_with_name_and_flags(None, MFD_CLOEXEC)
}

/// `memfd_set_seal(2)` — apply `seals` (`F_SEAL_*`) to an existing memfd so it
/// can no longer be modified or deleted in the ways those flags forbid.
///
/// Requires the memfd to have been created with `MFD_ALLOW_SEALING`. Returns
/// `Ok(())` on success.
pub fn memfd_set_seal(fd: &LocalFd, seals: u32) -> Result<(), SyscallError> {
    // The mask is a small non-negative bitfield, so the `i32` variadic
    // argument carries it losslessly.
    // SAFETY: `F_ADD_SEALS` passes the seal mask by value and dereferences no
    // pointer; `cvt` maps the -1 return to a `SyscallError`.
    cvt(unsafe { libc::fcntl(fd.as_raw(), F_ADD_SEALS, seals as i32) as isize })?;
    Ok(())
}
