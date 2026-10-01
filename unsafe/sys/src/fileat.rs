use crate::AtFd;
use core::ffi::CStr;

pub const AT_REMOVEDIR: i32 = libc::AT_REMOVEDIR;
pub const AT_SYMLINK_NOFOLLOW: i32 = libc::AT_SYMLINK_NOFOLLOW;
pub const RENAME_NOREPLACE: u32 = libc::RENAME_NOREPLACE;
pub const RENAME_EXCHANGE: u32 = libc::RENAME_EXCHANGE;
pub const RENAME_WHITEOUT: u32 = libc::RENAME_WHITEOUT;
pub const UTIME_NOW: i64 = libc::UTIME_NOW;
pub const UTIME_OMIT: i64 = libc::UTIME_OMIT;

/// A timestamp, mirroring the uapi `struct timespec` for `utimensat`.
///
/// `tv_sec` is epoch seconds, `tv_nsec` nanoseconds. For the special values
/// (`UTIME_NOW`/`UTIME_OMIT`) the kernel requires `tv_sec == 0` with the marker
/// in `tv_nsec`.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

pub fn mkdirat(dirfd: AtFd<'_>, pathname: &CStr, mode: u32) -> Result<(), crate::SyscallError> {
    let dirfd = dirfd.as_raw();
    // SAFETY: `mkdirat` with an invalid dirfd/path returns the appropriate errno.
    // `mode` is a bitmask; any value is accepted by the kernel (bits are masked).
    crate::cvt(unsafe { libc::mkdirat(dirfd, pathname.as_ptr(), mode as libc::mode_t) as isize })?;
    Ok(())
}

pub fn mkfifoat(dirfd: AtFd<'_>, pathname: &CStr, mode: u32) -> Result<(), crate::SyscallError> {
    let dirfd = dirfd.as_raw();
    // SAFETY: `mkfifoat` with an invalid dirfd/path returns the appropriate errno.
    // `mode` is a bitmask; the kernel keeps only the permission bits.
    crate::cvt(unsafe { libc::mkfifoat(dirfd, pathname.as_ptr(), mode as libc::mode_t) as isize })?;
    Ok(())
}

pub fn renameat2(
    olddirfd: AtFd<'_>,
    oldpath: &CStr,
    newdirfd: AtFd<'_>,
    newpath: &CStr,
    flags: u32,
) -> Result<(), crate::SyscallError> {
    let olddirfd = olddirfd.as_raw();
    let newdirfd = newdirfd.as_raw();
    // SAFETY: renameat2 with invalid fds/paths returns the appropriate errno.
    crate::cvt(unsafe {
        libc::renameat2(
            olddirfd,
            oldpath.as_ptr(),
            newdirfd,
            newpath.as_ptr(),
            flags,
        ) as isize
    })?;
    Ok(())
}

pub fn unlinkat(dirfd: AtFd<'_>, pathname: &CStr, flags: i32) -> Result<(), crate::SyscallError> {
    let dirfd = dirfd.as_raw();
    // SAFETY: `unlinkat` with invalid fd/path returns the appropriate errno.
    // `flags` is 0 (unlink) or AT_REMOVEDIR (rmdir); any other value is rejected.
    crate::cvt(unsafe { libc::unlinkat(dirfd, pathname.as_ptr(), flags) as isize })?;
    Ok(())
}

pub fn symlinkat(
    oldpath: &CStr,
    newdirfd: AtFd<'_>,
    newpath: &CStr,
) -> Result<(), crate::SyscallError> {
    let newdirfd = newdirfd.as_raw();
    // SAFETY: `symlinkat` with an invalid dirfd/path returns the appropriate errno.
    // The link content (`oldpath`) is stored verbatim and never resolved by the kernel.
    crate::cvt(unsafe { libc::symlinkat(oldpath.as_ptr(), newdirfd, newpath.as_ptr()) as isize })?;
    Ok(())
}

pub fn utimensat(
    dirfd: AtFd<'_>,
    path: &CStr,
    atime: &Timespec,
    mtime: &Timespec,
    flags: i32,
) -> Result<(), crate::SyscallError> {
    let dirfd = dirfd.as_raw();
    // `libc::utimensat` takes a pointer to a two-element `timespec` array
    // (atime, mtime). `Timespec` mirrors that layout on x86_64, so the array
    // is passed through a cast.
    let times = [*atime, *mtime];
    // SAFETY: `utimensat` with an invalid dirfd/path returns the appropriate
    // errno. `Timespec` is `#[repr(C)]` with the exact field layout of
    // `libc::timespec` (two c_long on x86_64); `times` is a valid two-element
    // array and `flags` is 0 or AT_SYMLINK_NOFOLLOW.
    crate::cvt(unsafe {
        libc::utimensat(dirfd, path.as_ptr(), times.as_ptr().cast(), flags) as isize
    })?;
    Ok(())
}
