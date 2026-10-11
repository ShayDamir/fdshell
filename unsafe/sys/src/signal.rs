use crate::signalfd::SIGPIPE;
use crate::{LocalFd, Pid, cvt};

pub const SIGKILL: i32 = libc::SIGKILL;
pub const SIGTERM: i32 = libc::SIGTERM;

/// The default signal disposition: the kernel's own action for the signal.
/// Re-exported (as are the others here) so safe crates never touch `libc`,
/// and comparable with what [`sigpipe_disposition`] reports.
pub const SIG_DFL: usize = libc::SIG_DFL;
/// The disposition that discards the signal — what Rust's std installs for
/// `SIGPIPE`, and therefore what every fdshell child inherits.
pub const SIG_IGN: usize = libc::SIG_IGN;

/// Send signal `sig` to process `pid`.
pub fn kill(pid: Pid, sig: i32) -> Result<(), crate::SyscallError> {
    // SAFETY: `pid` is a valid pid; `sig` is a signal number; an unknown pid
    // returns -1/`ESRCH`, caught by `cvt`.
    crate::cvt(unsafe { libc::kill(pid.as_raw(), sig) as isize })?;
    Ok(())
}

impl LocalFd {
    /// Send signal `sig` to the process referenced by this pidfd.
    ///
    /// A simple signal is sent (`info == NULL`, `flags == 0`), so no `siginfo_t`
    /// is constructed. `self` must be a valid pidfd (e.g. from `clone3`).
    pub fn send_signal(&self, sig: i32) -> Result<(), crate::SyscallError> {
        // SAFETY: SYS_pidfd_send_signal (424) is valid on Linux ≥5.1 x86_64.
        // `self` is a valid pidfd; `sig` is a signal number; NULL info with
        // flags 0 sends a simple signal, so no extra memory is dereferenced.
        crate::cvt(unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.as_raw() as i64,
                sig as i64,
                0usize,
                0u64,
            ) as isize
        })?;
        Ok(())
    }
}

/// Restore `SIGPIPE` to [`SIG_DFL`] and unblock it in the calling process.
///
/// Both halves are needed. Rust's std installs `SIG_IGN` for `SIGPIPE` at
/// startup, and `clone3` inherits the disposition *and* the signal mask, so
/// without this a forked pipeline stage gets `EPIPE` from a write to a pipe
/// with no readers and reports it as a `write error`; `signalfd %v PIPE` adds
/// `SIGPIPE` to the mask on top of that, where `sigaction` alone would leave
/// the write failing with `EPIPE`. A shell child must die of the signal,
/// silently, status 141, the way a shell stage should.
pub fn reset_sigpipe() -> Result<(), crate::SyscallError> {
    let action = libc::sigaction {
        sa_sigaction: SIG_DFL,
        // SAFETY: `sigset_t` is a plain byte array; zeroed is a valid (empty) set.
        sa_mask: unsafe { core::mem::zeroed() },
        sa_flags: 0,
        sa_restorer: None,
    };
    // SAFETY: `SIGPIPE` is a valid signal number; `action` is a fully
    // initialized `sigaction` with an empty mask, no handler pointer and no
    // `SA_RESTORER` flag; a NULL `oldact` is allowed.
    cvt(unsafe { libc::sigaction(SIGPIPE, &action, core::ptr::null_mut()) as isize })?;

    // SAFETY: `mask` is a plain byte array; zeroed is a valid (empty) set.
    let mut mask: libc::sigset_t = unsafe { core::mem::zeroed() };
    // SAFETY: `mask` is a valid `sigset_t`; `sigemptyset` always succeeds.
    unsafe { libc::sigemptyset(&mut mask) };
    // SAFETY: `mask` is a valid `sigset_t`; `SIGPIPE` is a valid signal
    // number; an invalid one returns -1/`EINVAL`, caught by `cvt`.
    cvt(unsafe { libc::sigaddset(&mut mask, SIGPIPE) as isize })?;
    // SAFETY: `mask` is a valid `sigset_t`; `SIG_UNBLOCK` only clears its
    // members in the calling thread's mask; a NULL old set requests no report.
    cvt(unsafe { libc::sigprocmask(libc::SIG_UNBLOCK, &mask, core::ptr::null_mut()) as isize })?;
    Ok(())
}

/// The disposition `SIGPIPE` currently has in the calling process, as its
/// `sa_sigaction` value ([`SIG_DFL`], [`SIG_IGN`], …). For tests that pin the
/// disposition a forked child ends up with.
pub fn sigpipe_disposition() -> Result<usize, crate::SyscallError> {
    // SAFETY: a zeroed `sigaction` is a valid all-clear value.
    let mut old: libc::sigaction = unsafe { core::mem::zeroed() };
    // SAFETY: `SIGPIPE` is a valid signal number; a NULL `act` means "query
    // only", which writes the current disposition into the valid `old`.
    cvt(unsafe { libc::sigaction(SIGPIPE, core::ptr::null(), &mut old) as isize })?;
    Ok(old.sa_sigaction)
}
