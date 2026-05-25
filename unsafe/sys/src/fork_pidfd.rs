use crate::{cvt, Fd};

pub fn fork_pidfd() -> Result<(isize, Option<Fd>), i32> {
    let mut pidfd_out: u64 = 0;
    // SAFETY: clone_args is integer types; zeroed is valid.
    let mut args: libc::clone_args = unsafe { core::mem::zeroed() };
    args.flags = libc::CLONE_PIDFD as u64;
    args.exit_signal = libc::SIGCHLD as u64;
    args.pidfd = (&raw mut pidfd_out) as u64;

    // SAFETY: SYS_clone3 (435) is valid on Linux ≥5.3 x86_64.
    // args and pidfd_out are valid stack allocations.
    let ret = cvt(unsafe {
        libc::syscall(
            libc::SYS_clone3,
            &raw mut args,
            core::mem::size_of_val(&args),
        ) as isize
    })?;

    if ret == 0 {
        return Ok((0, None));
    }
    let raw = pidfd_out as i32;
    // SAFETY: `raw` is a valid fd from clone3; setting CLOEXEC is well-defined.
    if unsafe { libc::fcntl(raw, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        let err = unsafe { *libc::__errno_location() };
        unsafe { libc::close(raw) };
        return Err(err);
    }
    // SAFETY: `raw` has CLOEXEC confirmed above.
    let pidfd = unsafe { Fd::from_raw(raw) };
    Ok((ret, Some(pidfd)))
}
