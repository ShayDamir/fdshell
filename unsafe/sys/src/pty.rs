use crate::LocalFd;

/// Allocate a pseudo-terminal pair; returns `(master, slave)`. The slave is a
/// terminal device, so `tty::isatty` on it is true. Both ends get CLOEXEC.
pub fn openpty() -> Result<(LocalFd, LocalFd), crate::SyscallError> {
    let mut master: libc::c_int = 0;
    let mut slave: libc::c_int = 0;
    // SAFETY: `openpty` writes both fds into the (valid) out-params; null
    // name/termios/winsize are accepted; a failure returns -1, caught by `cvt`.
    crate::cvt(unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            core::ptr::null_mut(),
            core::ptr::null(),
            core::ptr::null(),
        ) as isize
    })?;
    for fd in [master, slave] {
        // SAFETY: `F_SETFD` on a valid fd (checked by `cvt`) only sets a flag
        // and cannot invalidate the fd.
        crate::cvt(unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) as isize })?;
    }
    // SAFETY: CLOEXEC is set on both, satisfying the `LocalFd` invariant.
    Ok((unsafe { LocalFd::from_raw(master) }, unsafe {
        LocalFd::from_raw(slave)
    }))
}
