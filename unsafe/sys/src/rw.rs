use crate::Fd;

pub fn read(fd: &Fd, buf: &mut [u8]) -> Result<isize, i32> {
    // SAFETY: `buf` is a valid mutable slice; `read` won't write past `buf.len()`.
    crate::cvt(unsafe {
        libc::read(
            fd.as_raw(),
            buf.as_mut_ptr() as *mut core::ffi::c_void,
            buf.len(),
        )
    })
}

pub fn write(fd: &Fd, buf: &[u8]) -> Result<isize, i32> {
    // SAFETY: `buf` is a valid immutable slice; `write` won't read past `buf.len()`.
    crate::cvt(unsafe {
        libc::write(
            fd.as_raw(),
            buf.as_ptr() as *const core::ffi::c_void,
            buf.len(),
        )
    })
}
