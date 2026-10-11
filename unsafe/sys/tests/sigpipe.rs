#![allow(clippy::unwrap_used)]

use sys::SyscallError;
use sys::fork_cell::ForkCell;
use sys::fork_pidfd::fork_pidfd_cell;
use sys::pipe::pipe2;
use sys::siginfo::WaitStatus;
use sys::signal::{SIG_DFL, SIG_IGN, sigpipe_disposition};
use sys::signalfd::{self, SIGPIPE};

/// Fork a child with [`fork_pidfd_cell`], let it report one `usize` over a
/// pipe, and return what the parent read.
///
/// The report closure runs **in the child**, so it measures the child's own
/// process state. Nothing panics in the child (a forked test harness catches a
/// panic and the child would still exit 0 — LESSONS), so a failed write
/// travels as exit 1 and the parent's reap assertion reports it.
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
        errno: sys::errno::EINVAL,
        syscall: "fork_pidfd_cell",
    })?;
    match pidfd.wait_pidfd()? {
        WaitStatus::Exited(0) => {}
        other => {
            // The child's report failed (a panic it swallowed, or a write
            // error): report the status, because the byte read is meaningless.
            eprintln!("child exited with status {}", other.exit_code());
            return Err(SyscallError::Other {
                errno: sys::errno::EINVAL,
                syscall: "child report exit",
            });
        }
    }
    assert_eq!(n, 8, "child reported {n} of 8 bytes");
    Ok(usize::from_ne_bytes(buf))
}

/// A pipeline stage must run with `SIGPIPE` at its default disposition, so a
/// write to a pipe with no readers kills it instead of returning `EPIPE`.
#[test]
fn forked_child_runs_with_default_sigpipe() -> Result<(), SyscallError> {
    // Pin Rust std's startup install: a std process ignores SIGPIPE. A
    // toolchain that stops doing it fails here loudly, instead of silently
    // leaving the fork reset masking a different process state.
    assert_eq!(
        sigpipe_disposition()?,
        SIG_IGN,
        "std must start with SIGPIPE ignored — the fork reset is what children rely on"
    );
    // The plain fork (no `fork_pidfd_cell`) inherits the parent's disposition,
    // which is what test-only fork children rely on: the shell's own writes
    // stay safe from a closed stdout.
    assert_eq!(
        child_reports(|| sigpipe_disposition().unwrap_or(SIG_IGN))?,
        SIG_DFL,
        "a forked child must run with SIGPIPE at SIG_DFL"
    );
    Ok(())
}

/// `signalfd %v PIPE` blocks SIGPIPE in the shell, and `clone3` inherits the
/// mask, so restoring the disposition alone is not enough — the child must
/// also have the signal unblocked, or its write still fails with `EPIPE`.
/// Both halves of the reset are pinned here, in the child's own process state.
#[test]
fn forked_child_of_a_shell_with_blocked_sigpipe_is_fully_reset() -> Result<(), SyscallError> {
    let _fd = signalfd::signalfd(&[SIGPIPE], 0)?;
    // Pin the signalfd contract first: the parent really blocks the signal
    // (that is how the signalfd receives it), so the child's clean state can
    // only come from the reset.
    assert!(
        signalfd::signal_blocked(SIGPIPE),
        "signalfd must block SIGPIPE in the parent"
    );
    assert_eq!(
        child_reports(|| usize::from(signalfd::signal_blocked(SIGPIPE)))?,
        0,
        "the mask half of the reset is missing"
    );
    assert_eq!(
        child_reports(|| sigpipe_disposition().unwrap_or(SIG_IGN))?,
        SIG_DFL,
        "the disposition half of the reset is missing"
    );
    Ok(())
}
