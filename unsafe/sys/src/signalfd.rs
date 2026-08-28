use crate::LocalFd;

pub const SFD_NONBLOCK: i32 = libc::SFD_NONBLOCK;
pub const SFD_CLOEXEC: i32 = libc::SFD_CLOEXEC;

/// Signal numbers usable with [`signalfd`] (re-exported so safe crates never
/// touch `libc`).
pub const SIGHUP: i32 = libc::SIGHUP;
pub const SIGINT: i32 = libc::SIGINT;
pub const SIGQUIT: i32 = libc::SIGQUIT;
pub const SIGILL: i32 = libc::SIGILL;
pub const SIGTRAP: i32 = libc::SIGTRAP;
pub const SIGABRT: i32 = libc::SIGABRT;
pub const SIGBUS: i32 = libc::SIGBUS;
pub const SIGFPE: i32 = libc::SIGFPE;
pub const SIGSEGV: i32 = libc::SIGSEGV;
pub const SIGPIPE: i32 = libc::SIGPIPE;
pub const SIGALRM: i32 = libc::SIGALRM;
pub const SIGTERM: i32 = libc::SIGTERM;
pub const SIGCHLD: i32 = libc::SIGCHLD;
pub const SIGCONT: i32 = libc::SIGCONT;
pub const SIGSTOP: i32 = libc::SIGSTOP;
pub const SIGTSTP: i32 = libc::SIGTSTP;
pub const SIGTTIN: i32 = libc::SIGTTIN;
pub const SIGTTOU: i32 = libc::SIGTTOU;
pub const SIGURG: i32 = libc::SIGURG;
pub const SIGXCPU: i32 = libc::SIGXCPU;
pub const SIGXFSZ: i32 = libc::SIGXFSZ;
pub const SIGVTALRM: i32 = libc::SIGVTALRM;
pub const SIGPROF: i32 = libc::SIGPROF;
pub const SIGWINCH: i32 = libc::SIGWINCH;
pub const SIGIO: i32 = libc::SIGIO;
pub const SIGPWR: i32 = libc::SIGPWR;
pub const SIGSYS: i32 = libc::SIGSYS;
pub const SIGUSR1: i32 = libc::SIGUSR1;
pub const SIGUSR2: i32 = libc::SIGUSR2;

/// Create a signalfd that becomes readable when any signal in `signals` is
/// delivered, with `CLOEXEC` set. The signals are also blocked in the calling
/// thread (via `sigprocmask`), which is required for the signalfd to receive
/// them instead of the default disposition.
pub fn signalfd(signals: &[i32], flags: i32) -> Result<LocalFd, crate::SyscallError> {
    // SAFETY: `sigset_t` is a plain byte array; zeroed is a valid (empty) set.
    let mut mask: libc::sigset_t = unsafe { core::mem::zeroed() };
    // SAFETY: `mask` is a valid `sigset_t`; `sigemptyset` always succeeds.
    unsafe { libc::sigemptyset(&mut mask) };
    for &sig in signals {
        // SAFETY: `mask` is valid; `sig` is a signal number; an invalid
        // signal returns -1/`EINVAL`, caught by `cvt`.
        crate::cvt(unsafe { libc::sigaddset(&mut mask, sig) as isize })?;
    }
    // SAFETY: `mask` is valid; `sigprocmask` blocks the signals in this
    // thread so the signalfd receives them.
    crate::cvt(unsafe {
        libc::sigprocmask(libc::SIG_BLOCK, &mask, core::ptr::null_mut()) as isize
    })?;
    // SAFETY: `mask` is a valid `sigset_t`; `signalfd(-1, ...)` creates a new
    // fd with `CLOEXEC`, checked by `cvt`.
    let ret = crate::cvt(unsafe { libc::signalfd(-1, &mask, flags | SFD_CLOEXEC) as isize })?;
    // SAFETY: `SFD_CLOEXEC` is set, satisfying the `LocalFd` invariant.
    Ok(unsafe { LocalFd::from_raw(ret as i32) })
}

/// Check whether `sig` is blocked in the calling thread's signal mask.
pub fn signal_blocked(sig: i32) -> bool {
    // SAFETY: `sigset_t` is a plain byte array; zeroed is a valid (empty) set.
    let mut mask: libc::sigset_t = unsafe { core::mem::zeroed() };
    // SAFETY: `mask` is valid; `sigprocmask` with a null new set only reads
    // the current mask.
    unsafe { libc::sigprocmask(libc::SIG_BLOCK, core::ptr::null_mut(), &mut mask) };
    // SAFETY: `mask` is valid; `sig` is a signal number.
    unsafe { libc::sigismember(&mask, sig) != 0 }
}
