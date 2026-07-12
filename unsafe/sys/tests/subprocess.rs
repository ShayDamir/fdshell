use sys::SyscallError;
use sys::siginfo::WaitStatus;

#[test]
fn fork_exit_0() -> Result<(), SyscallError> {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
    if ret == 0 {
        std::process::exit(0);
    }
    let pidfd = pidfd_opt.ok_or(SyscallError::Other {
        errno: sys::errno::EINVAL,
        syscall: "fork_pidfd",
    })?;
    pidfd.verify()?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(0) => Ok(()),
        _ => Err(SyscallError::Other {
            errno: sys::errno::EINVAL,
            syscall: "wait_pidfd",
        }),
    }
}

#[test]
fn fork_exit_42() -> Result<(), SyscallError> {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
    if ret == 0 {
        std::process::exit(42);
    }
    let pidfd = pidfd_opt.ok_or(SyscallError::Other {
        errno: sys::errno::EINVAL,
        syscall: "fork_pidfd",
    })?;
    pidfd.verify()?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(42) => Ok(()),
        _ => Err(SyscallError::Other {
            errno: sys::errno::EINVAL,
            syscall: "wait_pidfd",
        }),
    }
}

#[test]
fn fork_signaled() -> Result<(), SyscallError> {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
    if ret == 0 {
        // SAFETY: raise sends SIGKILL to ourselves, which terminates the child.
        unsafe { libc::raise(libc::SIGKILL) };
        std::process::exit(0);
    }
    let pidfd = pidfd_opt.ok_or(SyscallError::Other {
        errno: sys::errno::EINVAL,
        syscall: "fork_pidfd",
    })?;
    pidfd.verify()?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Signaled(libc::SIGKILL) => Ok(()),
        _ => Err(SyscallError::Other {
            errno: sys::errno::EINVAL,
            syscall: "wait_pidfd",
        }),
    }
}
