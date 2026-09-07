use crate::{AtFd, SyscallError, cvt};
use core::ffi::CStr;

// `Debug` + `PartialEq` are derived plainly (not `#[cfg(test)]`) because the
// integration tests in `unsafe/sys/tests/` call `unwrap_err()` on the `Ok`
// value while `sys` is compiled as a dependency (no test cfg). All fields are
// primitives, so no trait-propagation concern.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Statx {
    pub ino: u64,
    pub mode: u32,
    pub size: u64,
    pub dev_major: u32,
    pub dev_minor: u32,
    pub mtime_sec: i64,
}

/// `statx(2)` metadata for `path` relative to `dirfd` (CWD when `AtFd::cwd`).
///
/// `flags` may include `AT_SYMLINK_NOFOLLOW` (stat the link itself) and
/// `AT_EMPTY_PATH` (with an open handle in `dirfd` and an empty `path`,
/// re-stat that same handle).
pub fn statx(dirfd: AtFd<'_>, path: &CStr, flags: i32) -> Result<Statx, SyscallError> {
    // SAFETY: zero-initialized `libc::statx` is valid (all integer fields).
    let mut raw: libc::statx = unsafe { core::mem::zeroed() };
    // SAFETY: SYS_statx (332) is valid on Linux x86_64. `dirfd` is AT_FDCWD or
    // an open fd; `path` is a valid C string read only by the kernel; `raw`
    // is a valid stack buffer the syscall writes.
    cvt(unsafe {
        libc::syscall(
            libc::SYS_statx,
            dirfd.as_raw() as i64,
            path.as_ptr(),
            flags as i64,
            libc::STATX_BASIC_STATS as i64,
            &raw mut raw,
        ) as isize
    })?;
    Ok(Statx {
        ino: raw.stx_ino,
        mode: raw.stx_mode as u32,
        size: raw.stx_size,
        dev_major: raw.stx_dev_major,
        dev_minor: raw.stx_dev_minor,
        mtime_sec: raw.stx_mtime.tv_sec,
    })
}
