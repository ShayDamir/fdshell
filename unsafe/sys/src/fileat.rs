use crate::AtFd;
use core::ffi::CStr;

pub const AT_REMOVEDIR: i32 = libc::AT_REMOVEDIR;
pub const RENAME_NOREPLACE: u32 = libc::RENAME_NOREPLACE;
pub const RENAME_EXCHANGE: u32 = libc::RENAME_EXCHANGE;
pub const RENAME_WHITEOUT: u32 = libc::RENAME_WHITEOUT;

pub fn mkdirat(dirfd: AtFd<'_>, pathname: &CStr, mode: u32) -> Result<(), crate::SyscallError> {
    let dirfd = dirfd.as_raw();
    // SAFETY: `mkdirat` with an invalid dirfd/path returns the appropriate errno.
    // `mode` is a bitmask; any value is accepted by the kernel (bits are masked).
    crate::cvt(unsafe { libc::mkdirat(dirfd, pathname.as_ptr(), mode as libc::mode_t) as isize })?;
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
