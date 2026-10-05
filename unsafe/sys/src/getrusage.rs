use crate::SyscallError;

/// CPU times in microseconds: `utime` (user) and `stime` (system).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTimes {
    pub utime: u64,
    pub stime: u64,
}

impl CpuTimes {
    /// The own-process fields of a `libc::rusage`, in microseconds.
    pub fn from_rusage(raw: &libc::rusage) -> Self {
        CpuTimes {
            utime: us(raw.ru_utime),
            stime: us(raw.ru_stime),
        }
    }
}

/// `getrusage(RUSAGE_SELF)`: the calling process's own user and system times.
///
/// Linux's `struct rusage` carries no reaped-children fields (a BSD
/// extension), so children times are accumulated by the caller from the
/// `rusage` out-parameter of each `waitid` reap (see
/// [`crate::LocalFd::wait_pidfd_rusage`]).
pub fn self_usage() -> Result<CpuTimes, SyscallError> {
    // SAFETY: `libc::rusage` is all-integer; zeroed memory is valid.
    let mut raw: libc::rusage = unsafe { core::mem::zeroed() };
    // SAFETY: `RUSAGE_SELF` is a legal input; `raw` is writable memory of the
    // right size for the kernel.
    crate::cvt(unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut raw) as isize })?;
    Ok(CpuTimes::from_rusage(&raw))
}

fn us(t: libc::timeval) -> u64 {
    (t.tv_sec as u64) * 1_000_000 + (t.tv_usec as u64)
}
