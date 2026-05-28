use core::sync::atomic::{AtomicU8, Ordering};

use crate::{LocalFd, cvt};

const UNKNOWN: u8 = 0;
const AUTO: u8 = 1;
const MANUAL: u8 = 2;
static PIDFD_CLOEXEC: AtomicU8 = AtomicU8::new(UNKNOWN);

pub fn fork_pidfd() -> Result<(isize, Option<LocalFd>), i32> {
    let mut raw_pidfd: i32 = -1;
    // SAFETY: clone_args is integer types; zeroed is valid.
    let mut args: libc::clone_args = unsafe { core::mem::zeroed() };
    args.flags = libc::CLONE_PIDFD as u64;
    args.exit_signal = libc::SIGCHLD as u64;
    args.pidfd = (&raw mut raw_pidfd) as u64;

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
        return Ok((0, None));
    }

    let state = PIDFD_CLOEXEC.load(Ordering::Relaxed);
    if state == UNKNOWN {
        // Probe whether clone3 sets CLOEXEC automatically.
        let flags =
            crate::cvt(unsafe { libc::fcntl(raw_pidfd, libc::F_GETFD) as isize }).unwrap_or(0);
        if flags & libc::FD_CLOEXEC as isize != 0 {
            PIDFD_CLOEXEC.store(AUTO, Ordering::Relaxed);
        } else {
            PIDFD_CLOEXEC.store(MANUAL, Ordering::Relaxed);
            if let Err(e) = crate::cvt(unsafe {
                libc::fcntl(raw_pidfd, libc::F_SETFD, libc::FD_CLOEXEC) as isize
            }) {
                unsafe { libc::close(raw_pidfd) };
                return Err(e);
            }
        }
    } else if state == MANUAL
        && let Err(e) =
            crate::cvt(unsafe { libc::fcntl(raw_pidfd, libc::F_SETFD, libc::FD_CLOEXEC) as isize })
    {
        unsafe { libc::close(raw_pidfd) };
        return Err(e);
    }
    // state == AUTO: kernel already set CLOEXEC, nothing to do.

    // SAFETY: `raw_pidfd` has CLOEXEC (set by kernel or fcntl above).
    let pidfd = unsafe { LocalFd::from_raw(raw_pidfd) };
    Ok((ret, Some(pidfd)))
}
