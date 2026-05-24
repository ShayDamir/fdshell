pub fn renameat(
    olddirfd: crate::AtFd<'_>,
    oldpath: &core::ffi::CStr,
    newdirfd: crate::AtFd<'_>,
    newpath: &core::ffi::CStr,
) -> Result<(), i32> {
    let olddirfd = olddirfd.as_raw();
    let newdirfd = newdirfd.as_raw();
    // SAFETY: `renameat` with invalid fds/paths returns the appropriate errno.
    crate::cvt(unsafe {
        libc::renameat(olddirfd, oldpath.as_ptr(), newdirfd, newpath.as_ptr()) as isize
    })?;
    Ok(())
}
