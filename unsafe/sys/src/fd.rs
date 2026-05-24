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

pub fn dup3(old: Fd, new: Fd) -> Result<(), i32> {
    // SAFETY: `dup3` with invalid fds or flags returns `EBADF`/`EINVAL`.
    crate::cvt(unsafe { libc::dup3(old, new, libc::O_CLOEXEC) as isize })?;
    Ok(())
}
