use crate::AtFd;

pub const RENAME_NOREPLACE: u32 = libc::RENAME_NOREPLACE;
pub const RENAME_EXCHANGE: u32 = libc::RENAME_EXCHANGE;
pub const RENAME_WHITEOUT: u32 = libc::RENAME_WHITEOUT;

pub fn renameat2(
    olddirfd: AtFd<'_>,
    oldpath: &core::ffi::CStr,
    newdirfd: AtFd<'_>,
    newpath: &core::ffi::CStr,
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
