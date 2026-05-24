use crate::Fd;

pub fn renameat(
    olddirfd: Fd,
    oldpath: &core::ffi::CStr,
    newdirfd: Fd,
    newpath: &core::ffi::CStr,
) -> Result<(), i32> {
    // SAFETY: `renameat` with invalid fds/paths returns the appropriate errno.
    crate::cvt(unsafe {
        libc::renameat(olddirfd, oldpath.as_ptr(), newdirfd, newpath.as_ptr()) as isize
    })?;
    Ok(())
}
