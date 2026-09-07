//! `memfd_create(2)` / `memfd_set_seal(2)` anonymous in-memory files.
//!
//! A memfd is an anonymous file that lives in RAM (or swap) under a name with
//! no filesystem entry, so it needs no directory, leaks no path, and vanishes
//! when its last fd closes. It backs temp-file-less heredocs and sealed
//! secrets: a memfd created with `MFD_ALLOW_SEALING` can have `F_SEAL_*` flags
//! applied via `memfd_set_seal` to forbid shrinking, growing, writing, or
//! mapping it, after which even the owner can no longer change it.
//!
//! `memfd_create` uses its `libc` wrapper; `memfd_set_seal` is an `ioctl`
//! (`F_SEAL_ADD`) rather than a libc function, so it is invoked by syscall
//! number.

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

/// `F_SEAL_ADD` — the ioctl command `memfd_set_seal` is built on
/// (`_IOW(0xfe, 0x1, unsigned)` from `<linux/fcntl.h>`), decomposed with the
/// x86_64 `_IOC_*` shifts rather than written as a bare magic constant.
const F_SEAL_ADD: u64 = (1u64 << 30) | (0xfeu64 << 8) | (1u64 << 0) | (4u64 << 16);

/// Create an anonymous in-memory file with the given name and flags.
///
/// `name` may be `None` for a fully anonymous memfd; a name is visible in
/// `/proc/<pid>/maps` and must be ≤ 14 bytes with no `/` or NUL (validated by
/// the caller). `flags` must include `MFD_CLOEXEC` (for a [`LocalFd`]) and, to
/// allow sealing, `MFD_ALLOW_SEALING`.
pub fn memfd_create_with_name_and_flags(
    name: Option<&CStr>,
    flags: u32,
) -> Result<LocalFd, SyscallError> {
    let ptr = match name {
        Some(c) => c.as_ptr(),
        None => core::ptr::null(),
    };
    // SAFETY: `ptr` is either null (anonymous) or a valid NUL-terminated C
    // string; `memfd_create` writes one fd or -1, checked by `cvt`; `MFD_CLOEXEC`
    // is the caller's responsibility and is satisfied by the callers.
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
    // A sealed memfd is immutable, so the ioctl reads (never writes) the buffer.
    // `unsigned int` is 4 bytes on Linux, so the mask is carried by a `u32` to
    // match the ioctl argument exactly.
    let mut seals_buf = seals;
    // SAFETY: `seals_buf` is a valid, readable `unsigned int`; the F_SEAL_ADD
    // ioctl takes a pointer to the seal mask and only reads it.
    cvt(unsafe {
        libc::syscall(
            libc::SYS_ioctl,
            fd.as_raw() as i64,
            F_SEAL_ADD as i64,
            &mut seals_buf as *mut u32 as *mut core::ffi::c_void,
        ) as isize
    })?;
    Ok(())
}
