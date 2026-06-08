pub fn fchmod(fd: i32, mode: u32) -> Result<(), i32> {
    // SAFETY: `fchmod` with an invalid fd returns `EBADF`.
    // It only modifies the file permissions of an open fd.
    crate::cvt(unsafe { libc::fchmod(fd, mode as libc::mode_t) as isize })?;
    Ok(())
}
