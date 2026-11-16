// mutants: -C --test -C kill -C --test -C pidfd_send_signal -C --test -C sigaction -- -E 'binary(kill) | binary(pidfd_send_signal) | binary(sigaction)'

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

        // An interrupted retry: the record handler runs with `sa_flags == 0`
        // (task #582's choice — no `SA_RESTART`), so a handled signal makes the
        // `waitid` answer `EINTR` **after** the handler has flagged it, and the
        // shell's flag-and-keep-walking pattern needs the wait to resume: the
        // pidfd still holds the child (measured, task #585 — a `kill -USR1 $$`
        // command used to abort the shell through this errno).
        loop {
            // SAFETY: SYS_waitid (247) is valid on x86_64 Linux. `self` is a valid
            // pidfd. info and raw are writable memory of the right size for the
            // kernel.
            match cvt(unsafe {
                libc::syscall(
                    libc::SYS_waitid,
                    libc::P_PIDFD as i64,
                    self.as_raw() as i64,
                    &raw mut info,
                    libc::WEXITED as i64,
                    &raw mut raw,
                ) as isize
            }) {
                Ok(_) => break,
                Err(SyscallError::Other { errno, .. }) if errno == libc::EINTR => continue,
                Err(e) => return Err(e),
            }
        }

        let status = match info.si_code {
            libc::CLD_EXITED => WaitStatus::Exited(info.si_status),
            libc::CLD_KILLED | libc::CLD_DUMPED => WaitStatus::Signaled(info.si_status),
            _ => return Err(SyscallError::EINVAL("waitid")),
        };
        Ok((status, CpuTimes::from_rusage(&raw)))
    }
}
