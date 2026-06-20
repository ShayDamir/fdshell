use crate::AtFd;

pub const AT_REMOVEDIR: i32 = libc::AT_REMOVEDIR;

pub fn unlinkat(
    dirfd: AtFd<'_>,
    pathname: &core::ffi::CStr,
    flags: i32,
) -> Result<(), crate::SyscallError> {
    let dirfd = dirfd.as_raw();
    // SAFETY: `unlinkat` with invalid fd/path returns the appropriate errno.
    // `flags` is 0 (unlink) or AT_REMOVEDIR (rmdir); any other value is rejected.
    crate::cvt(unsafe { libc::unlinkat(dirfd, pathname.as_ptr(), flags) as isize })?;
    Ok(())
}
