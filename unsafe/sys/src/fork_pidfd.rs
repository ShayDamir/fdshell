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
    // SAFETY: kernel wrote a valid pidfd (with O_CLOEXEC) into pidfd_out.
    let pidfd = unsafe { Fd::from_raw(pidfd_out as i32) };
    Ok((ret, Some(pidfd)))
}
