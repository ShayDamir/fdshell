use crate::LocalFd;

pub const TFD_NONBLOCK: i32 = libc::TFD_NONBLOCK;
pub const TFD_CLOEXEC: i32 = libc::TFD_CLOEXEC;
pub const CLOCK_MONOTONIC: i32 = libc::CLOCK_MONOTONIC;

/// Create a `CLOCK_MONOTONIC` timerfd with `CLOEXEC` set.
pub fn timerfd_create(flags: i32) -> Result<LocalFd, crate::SyscallError> {
    // SAFETY: `timerfd_create` returns a new fd on success or -1 on error,
    // checked by `cvt`; `CLOCK_MONOTONIC` is a valid clockid.
    let ret =
        crate::cvt(unsafe { libc::timerfd_create(CLOCK_MONOTONIC, flags | TFD_CLOEXEC) as isize })?;
    // SAFETY: `TFD_CLOEXEC` is set, satisfying the `LocalFd` invariant.
    Ok(unsafe { LocalFd::from_raw(ret as i32) })
}

/// Arm the timerfd: `value` is the initial expiry, `interval` the repeat
/// period (zero for a one-shot timer). Each is `(sec, nsec)`.
pub fn timerfd_settime(
    fd: &LocalFd,
    value: (i64, i64),
    interval: (i64, i64),
) -> Result<(), crate::SyscallError> {
    let new = libc::itimerspec {
        it_value: libc::timespec {
            tv_sec: value.0,
            tv_nsec: value.1,
        },
        it_interval: libc::timespec {
            tv_sec: interval.0,
            tv_nsec: interval.1,
        },
    };
    // SAFETY: `fd` is a valid timerfd; `new` is a valid `itimerspec`; a null
    // `old_value` skips the read-back. Out-of-range values return `EINVAL`.
    crate::cvt(unsafe {
        libc::timerfd_settime(fd.as_raw(), 0, &new, core::ptr::null_mut()) as isize
    })?;
    Ok(())
}
