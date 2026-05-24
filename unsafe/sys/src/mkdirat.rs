use crate::Fd;

pub fn mkdirat(dirfd: Fd, pathname: &core::ffi::CStr, mode: u32) -> Result<(), i32> {
    // SAFETY: `mkdirat` with an invalid dirfd/path returns the appropriate errno.
    // `mode` is a bitmask; any value is accepted by the kernel (bits are masked).
    crate::cvt(unsafe {
        libc::mkdirat(dirfd, pathname.as_ptr(), mode as libc::mode_t) as isize
    })?;
    Ok(())
}
