use crate::fork_cell::ForkCell;
use crate::{LocalFd, Pid, cvt};

pub fn fork_pidfd() -> Result<(Pid, Option<LocalFd>), crate::SyscallError> {
    // SAFETY: `raw_pidfd` is a clone3 out-parameter; the kernel writes a valid
    // pidfd to it on the parent path before it is read via `assume_init`.
    let mut raw_pidfd = core::mem::MaybeUninit::<i32>::uninit();
    // SAFETY: clone_args is integer types; zeroed is valid.
    let mut args: libc::clone_args = unsafe { core::mem::zeroed() };
    args.flags = libc::CLONE_PIDFD as u64;
    args.exit_signal = libc::SIGCHLD as u64;
    args.pidfd = raw_pidfd.as_mut_ptr() as u64;

    // SAFETY: SYS_clone3 (435) is valid on Linux ≥5.3 x86_64.
    // args and raw_pidfd are valid stack allocations.
    let ret = cvt(unsafe {
        libc::syscall(
            libc::SYS_clone3,
            &raw mut args,
            core::mem::size_of_val(&args),
        ) as isize
    })?;

    if ret == 0 {
        return Ok((Pid::from_raw(0), None));
    }

    // SAFETY: on the parent path clone3 has written a valid pidfd into
    // `raw_pidfd` before returning, so the memory is initialized.
    let raw_pidfd = unsafe { raw_pidfd.assume_init() };

    // SAFETY: `raw_pidfd` is a valid fd from clone3; fcntl on an invalid
    // fd returns -1/EBADF, caught by `cvt`.
    cvt(unsafe { libc::fcntl(raw_pidfd, libc::F_SETFD, libc::FD_CLOEXEC) as isize })?;

    // SAFETY: `raw_pidfd` now has CLOEXEC (set by the kernel and reaffirmed
    // by fcntl above); the parent has exclusive ownership of the fd.
    let pidfd = unsafe { LocalFd::from_raw(raw_pidfd) };
    Ok((Pid::from_raw(ret as i32), Some(pidfd)))
}

/// Fork a subprocess and return a pidfd in the parent. Same return type as
/// [`fork_pidfd`], but calls `cell.reset_after_fork()` in the child process so
/// that `borrow_mut()` succeeds there.
pub fn fork_pidfd_cell<T>(
    cell: &ForkCell<T>,
) -> Result<(Pid, Option<LocalFd>), crate::SyscallError> {
    let (ret, pidfd_opt) = fork_pidfd()?;
    // Only the child (pid == 0, no pidfd) needs to reset its borrow counter.
    if ret.as_raw() == 0 {
        // SAFETY: we are in the forked child — exclusive ownership of memory.
        unsafe { cell.reset_after_fork() };
    }
    Ok((ret, pidfd_opt))
}
