use crate::getrusage::CpuTimes;
use crate::siginfo::{SigInfo, WaitStatus};
use crate::{LocalFd, SyscallError, cvt};

impl LocalFd {
    pub fn wait_pidfd(&self) -> Result<WaitStatus, SyscallError> {
        Ok(self.wait_pidfd_rusage()?.0)
    }

    /// Like [`wait_pidfd`], but also returns the reaped child's own CPU times
    /// (the `rusage` out-parameter of `waitid`), for `times` children
    /// accounting.
    pub fn wait_pidfd_rusage(&self) -> Result<(WaitStatus, CpuTimes), SyscallError> {
        // SAFETY: SigInfo is integer types; zeroed is valid.
        let mut info: SigInfo = unsafe { core::mem::zeroed() };
        // SAFETY: `libc::rusage` is all-integer; zeroed is valid.
        let mut raw: libc::rusage = unsafe { core::mem::zeroed() };

        // SAFETY: SYS_waitid (247) is valid on x86_64 Linux. `self` is a valid
        // pidfd. info and raw are writable memory of the right size for the
        // kernel.
        cvt(unsafe {
            libc::syscall(
                libc::SYS_waitid,
                libc::P_PIDFD as i64,
                self.as_raw() as i64,
                &raw mut info,
                libc::WEXITED as i64,
                &raw mut raw,
            ) as isize
        })?;

        let status = match info.si_code {
            libc::CLD_EXITED => WaitStatus::Exited(info.si_status),
            libc::CLD_KILLED | libc::CLD_DUMPED => WaitStatus::Signaled(info.si_status),
            _ => return Err(SyscallError::EINVAL("waitid")),
        };
        Ok((status, CpuTimes::from_rusage(&raw)))
    }
}
