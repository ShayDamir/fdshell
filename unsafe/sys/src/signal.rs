// mutants: -C --test -C kill -C --test -C pidfd_send_signal -C --test -C sigpipe -C --test -C sigaction -- -E 'binary(kill) | binary(pidfd_send_signal) | binary(sigpipe) | binary(sigaction)'

use crate::signalfd::SIGPIPE;
use crate::{LocalFd, Pid, cvt};
use core::sync::atomic::{AtomicU64, Ordering};

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

/// What [`set_action`] installs for one signal: the kernel's own action, a
/// discard, or the handler that only records the signal into [`PENDING`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Default,
    Ignore,
    Record,
}

/// Which signals [`record`] has seen: bit `sig - 1`, for signals `1..=64`
/// (Linux's whole range). One bit per signal, so the consumer learns *which*
/// trap fired, not merely that one did.
static PENDING: AtomicU64 = AtomicU64::new(0);

/// The handler [`Action::Record`] installs — dash's `onsig` / bash's
/// `trap_handler` shape: it records the pending signal and runs nothing else,
/// so the trap body stays out of the handler and runs later, in the executor.
///
/// It is async-signal-safe because this body is one atomic op: no allocation,
/// no syscall, no libc call. `sig` is `1..=64` — `sigaction` accepts nothing
/// else — so the shift is `0..=63`. The kernel passes three arguments to a
/// x86-64 handler; declaring only the first is the standard System V shape.
extern "C" fn record(sig: i32) {
    PENDING.fetch_or(1u64 << (sig - 1), Ordering::Relaxed);
}

/// Install `action` as the disposition of signal `sig` (`1..=64`).
///
/// `sa_flags` is `0` on purpose: without `SA_RESTART` a syscall interrupted by
/// a handled signal fails with `EINTR`, which is what the handler pattern
/// assumes; whether to restart is the consumer's decision. `sa_mask` is empty
/// and `sa_restorer` is `None`, the way a C program calls glibc's `sigaction`,
/// which fills the restorer in for a real handler.
pub fn set_action(sig: i32, action: Action) -> Result<(), crate::SyscallError> {
    let handler = match action {
        Action::Default => SIG_DFL,
        Action::Ignore => SIG_IGN,
        Action::Record => record as extern "C" fn(i32) as usize,
    };
    let act = libc::sigaction {
        sa_sigaction: handler,
        // SAFETY: `sigset_t` is a plain byte array; zeroed is a valid (empty) set.
        sa_mask: unsafe { core::mem::zeroed() },
        sa_flags: 0,
        sa_restorer: None,
    };
    // SAFETY: `act` is a fully initialized `sigaction` — a real handler or
    // `SIG_DFL`/`SIG_IGN`, empty mask, no `SA_RESTORER` flag — so glibc fills the
    // restorer itself; a NULL `oldact` is allowed. A signal outside `1..=64`
    // (and `0`, the "reset errno" signal) returns -1/`EINVAL`, caught by `cvt`.
    cvt(unsafe { libc::sigaction(sig, &act, core::ptr::null_mut()) as isize })?;
    Ok(())
}

/// The disposition `sig` currently has in the calling process, as its
/// `sa_sigaction` value ([`SIG_DFL`], [`SIG_IGN`], or a handler address).
pub fn disposition(sig: i32) -> Result<usize, crate::SyscallError> {
    // SAFETY: a zeroed `sigaction` is a valid all-clear value.
    let mut old: libc::sigaction = unsafe { core::mem::zeroed() };
    // SAFETY: `sig` is a signal number — an invalid one returns -1/`EINVAL`,
    // caught by `cvt`; a NULL `act` means "query only", which writes the
    // current disposition into the valid `old`.
    cvt(unsafe { libc::sigaction(sig, core::ptr::null(), &mut old) as isize })?;
    Ok(old.sa_sigaction)
}

/// Consume what [`record`] has flagged so far: return the mask of signals
/// recorded since the last call, and clear it.
///
/// Always `Ok` — there is no syscall here, so no errno can exist — but the
/// `Result` shape is the surface the safe crates get from this one.
pub fn take_pending() -> Result<u64, crate::SyscallError> {
    Ok(PENDING.swap(0, Ordering::Relaxed))
}
