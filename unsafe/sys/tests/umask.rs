use sys::SyscallError;
use sys::siginfo::WaitStatus;

fn with_exhausted_fds<T>(f: impl FnOnce() -> T) -> T {
    let mut held = Vec::new();
    loop {
        // SAFETY: dup(0) is safe with stdin; fails with EMFILE when table is full.
        let ret = unsafe { libc::dup(0) } as isize;
        match sys::cvt(ret) {
            Err(sys::SyscallError::EMFILE(_)) => break,
            Err(e) => panic!("unexpected errno from dup: {e}"),
            Ok(fd) => held.push(fd as i32),
        }
    }
    let result = f();
    for fd in held {
        // SAFETY: fd was returned by dup above and is still open.
        unsafe { libc::close(fd) };
    }
    result
}

#[test]
fn umask_save_restore() -> Result<(), SyscallError> {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
    if ret == 0 {
        sys::umask::init();
        let original = sys::umask::get();
        sys::umask::set(0o077);
        sys::umask::set(original);
        if sys::umask::get() != original {
            sys::exit(1);
        }
        sys::exit(0);
    }
    let pidfd = pidfd_opt.ok_or(sys::errno::EINVAL)?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(0) => Ok(()),
        other => Err(match other {
            WaitStatus::Exited(n) => SyscallError::Other {
                errno: n,
                syscall: "wait_pidfd",
            },
            _ => SyscallError::Other {
                errno: sys::errno::EINVAL,
                syscall: "wait_pidfd",
            },
        }),
    }
}

#[test]
fn umask_set_get() -> Result<(), SyscallError> {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
    if ret == 0 {
        sys::umask::init();
        sys::umask::set(0o077);
        if sys::umask::get() != 0o077 {
            sys::exit(1);
        }
        let prev = sys::umask::set(0o022);
        if prev != 0o077 {
            sys::exit(2);
        }
        if sys::umask::get() != 0o022 {
            sys::exit(3);
        }
        sys::exit(0);
    }
    let pidfd = pidfd_opt.ok_or(sys::errno::EINVAL)?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(0) => Ok(()),
        other => Err(match other {
            WaitStatus::Exited(n) => SyscallError::Other {
                errno: n,
                syscall: "wait_pidfd",
            },
            _ => SyscallError::Other {
                errno: sys::errno::EINVAL,
                syscall: "wait_pidfd",
            },
        }),
    }
}

#[test]
fn umask_set_get_zero() -> Result<(), SyscallError> {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
    if ret == 0 {
        sys::umask::init();
        let original = sys::umask::get();
        sys::umask::set(0o777);
        if sys::umask::get() != 0o777 {
            sys::exit(1);
        }
        sys::umask::set(original);
        sys::exit(0);
    }
    let pidfd = pidfd_opt.ok_or(sys::errno::EINVAL)?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(0) => Ok(()),
        other => Err(match other {
            WaitStatus::Exited(n) => SyscallError::Other {
                errno: n,
                syscall: "wait_pidfd",
            },
            _ => SyscallError::Other {
                errno: sys::errno::EINVAL,
                syscall: "wait_pidfd",
            },
        }),
    }
}

#[test]
fn umask_init_fallback_no_proc() -> Result<(), SyscallError> {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
    if ret == 0 {
        let ok = with_exhausted_fds(|| {
            sys::umask::init();
            let mask = sys::umask::get();
            mask != 0 && mask <= 0o777
        });
        sys::exit(if ok { 0 } else { 1 });
    }
    let pidfd = pidfd_opt.ok_or(sys::errno::EINVAL)?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(0) => Ok(()),
        other => Err(match other {
            WaitStatus::Exited(n) => SyscallError::Other {
                errno: n,
                syscall: "wait_pidfd",
            },
            _ => SyscallError::Other {
                errno: sys::errno::EINVAL,
                syscall: "wait_pidfd",
            },
        }),
    }
}
