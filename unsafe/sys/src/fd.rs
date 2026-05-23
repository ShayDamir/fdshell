use crate::Fd;

pub fn close(fd: Fd) -> Result<(), i32> {
    // SAFETY: `close` is safe to call with any `fd`; invalid fds return `EBADF`.
    crate::cvt(unsafe { libc::close(fd) as isize })?;
    Ok(())
}

pub fn dup2(old: Fd, new: Fd) -> Result<Fd, i32> {
    // SAFETY: `dup2` is safe to call with any `old`/`new` fds; invalid fds return `EBADF`.
    crate::cvt(unsafe { libc::dup2(old, new) as isize })?;
    Ok(new)
}

pub fn read(fd: Fd, buf: &mut [u8]) -> Result<isize, i32> {
    // SAFETY: `buf` is a valid mutable slice; `read` won't write past `buf.len()`.
    crate::cvt(unsafe {
        libc::read(fd, buf.as_mut_ptr() as *mut core::ffi::c_void, buf.len())
    })
}

pub fn write(fd: Fd, buf: &[u8]) -> Result<isize, i32> {
    // SAFETY: `buf` is a valid immutable slice; `write` won't read past `buf.len()`.
    crate::cvt(unsafe {
        libc::write(fd, buf.as_ptr() as *const core::ffi::c_void, buf.len())
    })
}

pub fn openat() -> i32 {
    todo!()
}

pub fn mkdirat() -> i32 {
    todo!()
}

pub fn renameat() -> i32 {
    todo!()
}
