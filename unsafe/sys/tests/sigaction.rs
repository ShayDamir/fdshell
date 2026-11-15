#![allow(clippy::unwrap_used)]

use sys::SyscallError;
use sys::errno;
use sys::fork_cell::ForkCell;
use sys::fork_pidfd::fork_pidfd_cell;
use sys::pipe::pipe2;
use sys::siginfo::WaitStatus;
use sys::signal::{Action, SIG_DFL, SIG_IGN, disposition, set_action, take_pending};
use sys::signalfd::{SIGUSR1, SIGUSR2};

/// The bit [`sys::signal`] reserves for signal `sig` in the pending mask.
fn bit(sig: i32) -> usize {
    1usize << (sig - 1)
}

/// Fork a child with [`fork_pidfd_cell`], let it report one `usize` over a
/// pipe, and return what the parent read.
///
/// Install, deliver and poll all run **in the child**: a signal disposition is
/// process-global state, so measuring it in the child keeps every row from
/// disturbing its neighbours. Nothing panics in the child (a forked test harness
/// catches a panic and the child would still exit 0 — LESSONS), so a failed
/// child-side check travels as exit 1 and the parent's reap assertion reports
/// it. The child allocates nothing: the harness's pipe and `ForkCell` are built
/// before the fork, and the 128 MB virtual-address cap is respected.
fn child_reports(report: impl FnOnce() -> usize) -> Result<usize, SyscallError> {
    let cell = ForkCell::new(0u8);
    let (rd, wr) = pipe2(0)?;
    let (ret, pidfd_opt) = fork_pidfd_cell(&cell)?;
    if ret.as_raw() == 0 {
        let value = report();
        if wr.write_all(&value.to_ne_bytes()).is_err() {
            sys::exit(1);
        }
        sys::exit(0);
    }

    // Parent: drop the write end, so the read returns 0 if the child dies
    // before writing anything instead of blocking forever.
    drop(wr);
    let mut buf = [0u8; 8];
    let n = rd.read(&mut buf)?;

    let pidfd = pidfd_opt.ok_or(SyscallError::Other {
        errno: errno::EINVAL,
        syscall: "fork_pidfd_cell",
    })?;
    match pidfd.wait_pidfd()? {
        WaitStatus::Exited(0) => {}
        other => {
            // The child's report failed (a panic it swallowed, a failed check,
            // or a write error): report the status, the byte read is meaningless.
            eprintln!("child exited with status {}", other.exit_code());
            return Err(SyscallError::Other {
                errno: errno::EINVAL,
                syscall: "child report exit",
            });
        }
    }
    assert_eq!(n, 8, "child reported {n} of 8 bytes");
    Ok(usize::from_ne_bytes(buf))
}

/// Deliver `sig` to the child itself, then cross one syscall boundary: the
/// kernel runs the handler on the return to userspace, so after `getpid` the
/// flag is certainly set.
///
/// A self-sent signal may already have been handled before `kill` returns, and
/// with `sa_flags == 0` (no `SA_RESTART`) POSIX lets the call answer `EINTR` —
/// so `kill`'s own result is not asserted here, the `sa_flags` choice is pinned
/// by [`flag_handler_records_the_signal`]'s exact-bit assert.
fn deliver_in_child(sig: i32) {
    let _ = sys::signal::kill(sys::env::getpid(), sig);
    let _ = sys::env::getpid();
}

/// The flag-setting handler must record the signal it was installed for, in its
/// own bit, and nothing else may happen inside it.
#[test]
fn flag_handler_records_the_signal() -> Result<(), SyscallError> {
    assert_eq!(
        child_reports(|| {
            set_action(SIGUSR1, Action::Record).unwrap();
            deliver_in_child(SIGUSR1);
            take_pending().unwrap() as usize
        })?,
        bit(SIGUSR1),
        "the handler must set exactly the delivered signal's bit"
    );
    Ok(())
}

/// Polling consumes: what the first poll reported is gone by the second one.
#[test]
fn poll_consumes() -> Result<(), SyscallError> {
    assert_eq!(
        child_reports(|| {
            set_action(SIGUSR1, Action::Record).unwrap();
            deliver_in_child(SIGUSR1);
            if take_pending().unwrap() != bit(SIGUSR1) as u64 {
                sys::exit(1);
            }
            take_pending().unwrap() as usize
        })?,
        0,
        "the second poll must report nothing — the first one consumed the flag"
    );
    Ok(())
}

/// The flags are per-signal, not one shared flag: the executor must be able to
/// tell which trap fired, so a delivery of SIGUSR2 reports only SIGUSR2's bit
/// even though SIGUSR1's handler is installed too.
#[test]
fn per_signal_flags_distinguish() -> Result<(), SyscallError> {
    assert_eq!(
        child_reports(|| {
            set_action(SIGUSR1, Action::Record).unwrap();
            set_action(SIGUSR2, Action::Record).unwrap();
            deliver_in_child(SIGUSR2);
            take_pending().unwrap() as usize
        })?,
        bit(SIGUSR2),
        "only the delivered signal's own bit may be set"
    );
    Ok(())
}

/// `Action::Ignore` installs the discard disposition, read back through a
/// `sigaction` query in the child that installed it (the style of the existing
/// SIGPIPE test).
#[test]
fn ignore_disposition_observed() -> Result<(), SyscallError> {
    assert_eq!(
        child_reports(|| {
            set_action(SIGUSR1, Action::Ignore).unwrap();
            disposition(SIGUSR1).unwrap()
        })?,
        SIG_IGN,
        "an installed Ignore must read back as SIG_IGN"
    );
    Ok(())
}

/// `Action::Default` installs the kernel's own disposition. Nothing is delivered
/// here: the default action for SIGUSR2 is to terminate the process.
#[test]
fn default_disposition_observed() -> Result<(), SyscallError> {
    assert_eq!(
        child_reports(|| {
            set_action(SIGUSR2, Action::Default).unwrap();
            disposition(SIGUSR2).unwrap()
        })?,
        SIG_DFL,
        "an installed Default must read back as SIG_DFL"
    );
    Ok(())
}

/// A signal outside `1..=64` (signal `0` is the "reset errno" signal, never a
/// signal number) fails through `cvt` instead of installing anything — the row
/// that pins the syscall path of `set_action`.
#[test]
fn invalid_signal_errors_via_cvt() {
    let err = match set_action(0, Action::Record) {
        Ok(()) => panic!("signal 0 must not install a disposition"),
        Err(err) => err,
    };
    assert_eq!(err.errno(), libc::EINVAL, "got {err}");
}
