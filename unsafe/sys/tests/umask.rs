use sys::siginfo::WaitStatus;

fn with_exhausted_fds<T>(f: impl FnOnce() -> T) -> T {
    let mut held = Vec::new();
    loop {
        // SAFETY: dup(0) is safe with stdin; fails with EMFILE when table is full.
        let ret = unsafe { libc::dup(0) } as isize;
        match sys::cvt(ret) {
            Err(sys::SyscallError::EMFILE) => break,
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
fn umask_save_restore() -> Result<(), i32> {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
    if ret == 0 {
        sys::umask::init();
        let original = sys::umask::get();
        sys::umask::set(0o077);
        sys::umask::set(original);
        if sys::umask::get() != original {
            std::process::exit(1);
        }
        std::process::exit(0);
    }
    let pidfd = pidfd_opt.ok_or(sys::errno::EINVAL)?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(0) => Ok(()),
        other => Err(match other {
            WaitStatus::Exited(n) => n,
            _ => sys::errno::EINVAL,
        }),
    }
}

#[test]
fn umask_set_get() -> Result<(), i32> {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
    if ret == 0 {
        sys::umask::init();
        sys::umask::set(0o077);
        if sys::umask::get() != 0o077 {
            std::process::exit(1);
        }
        let prev = sys::umask::set(0o022);
        if prev != 0o077 {
            std::process::exit(2);
        }
        if sys::umask::get() != 0o022 {
            std::process::exit(3);
        }
        std::process::exit(0);
    }
    let pidfd = pidfd_opt.ok_or(sys::errno::EINVAL)?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(0) => Ok(()),
        other => Err(match other {
            WaitStatus::Exited(n) => n,
            _ => sys::errno::EINVAL,
        }),
    }
}

#[test]
fn umask_set_get_zero() -> Result<(), i32> {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
    if ret == 0 {
        sys::umask::init();
        let original = sys::umask::get();
        sys::umask::set(0o777);
        if sys::umask::get() != 0o777 {
            std::process::exit(1);
        }
        sys::umask::set(original);
        std::process::exit(0);
    }
    let pidfd = pidfd_opt.ok_or(sys::errno::EINVAL)?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(0) => Ok(()),
        other => Err(match other {
            WaitStatus::Exited(n) => n,
            _ => sys::errno::EINVAL,
        }),
    }
}

#[test]
fn umask_init_fallback_no_proc() -> Result<(), i32> {
    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
    if ret == 0 {
        let ok = with_exhausted_fds(|| {
            sys::umask::init();
            let mask = sys::umask::get();
            mask != 0 && mask <= 0o777
        });
        std::process::exit(if ok { 0 } else { 1 });
    }
    let pidfd = pidfd_opt.ok_or(sys::errno::EINVAL)?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(0) => Ok(()),
        other => Err(match other {
            WaitStatus::Exited(n) => n,
            _ => sys::errno::EINVAL,
        }),
    }
}
