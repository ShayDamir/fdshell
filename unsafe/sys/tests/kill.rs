#![cfg_attr(test, allow(clippy::unwrap_used))]

use sys::siginfo::WaitStatus;
use sys::signalfd::SIGUSR1;

/// `kill` must actually deliver the signal. A child creates a signalfd for
/// SIGUSR1 (blocking it), signals readiness over a pipe, then polls the
/// signalfd and exits 42. The parent waits for readiness, sends SIGUSR1, and
/// checks the child exited 42. A stub `kill` returning `Ok(())` without
/// sending would leave the child polling forever.
#[test]
fn kill_delivers_signal_to_child() {
    let (rd, wr) = sys::pipe::pipe2(0).unwrap();
    let (child_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            // child
            drop(rd);
            let fd = sys::signalfd::signalfd(&[SIGUSR1], 0).unwrap();
            wr.write_all(b"r").unwrap();
            let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
            let n = sys::poll::poll(&mut pfd, 5000).unwrap();
            let got = n > 0 && pfd.first().unwrap().revents & sys::poll::POLLIN != 0;
            // Exit 42 only if the signal actually arrived; a stub `kill` that
            // never sends would leave the poll empty and the child exits 0.
            sys::exit(if got { 42 } else { 0 });
        }
        Some(pidfd) => {
            // parent
            drop(wr);
            let mut buf = [0u8; 1];
            rd.read(&mut buf).unwrap();
            sys::signal::kill(child_pid, SIGUSR1).unwrap();
            let status = pidfd.wait_pidfd().unwrap();
            assert!(
                matches!(status, WaitStatus::Exited(42)),
                "child must exit 42 after SIGUSR1"
            );
        }
    }
}

/// The `EINTR`-only guard of the `waitid` retry (task #585): a reaped child's
/// pidfd answers a *different* errno (`ECHILD`) on a second wait, and it must
/// propagate, not retry forever. An unconditional retry swallows it and the
/// wait spins.
#[test]
fn second_reap_of_the_same_child_reports_echild() {
    let (_child_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            sys::exit(7);
        }
        Some(pidfd) => {
            let first = pidfd.wait_pidfd().unwrap();
            assert!(
                matches!(first, WaitStatus::Exited(7)),
                "first reap must succeed"
            );
            let err = match pidfd.wait_pidfd() {
                Ok(_status) => panic!("the second reap of a reaped child must fail"),
                Err(err) => err,
            };
            assert_eq!(
                err.errno(),
                libc::ECHILD,
                "the guard retries only EINTR, got {err}"
            );
            let _ = _child_pid;
        }
    }
}

/// The `CLD_KILLED` arm: a child killed by `SIGUSR1` (default action: terminate)
/// reports `Signaled`, not the `EINVAL` fallback for unknown `si_code`s. Deleting
/// the arm sends it into that fallback.
#[test]
fn a_signal_killed_child_reports_signaled() {
    let (_child_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            // `Pid::from_raw(0)` on the child path, so signal *self*, not pid 0.
            sys::signal::kill(sys::env::getpid(), SIGUSR1).unwrap();
            sys::exit(0);
        }
        Some(pidfd) => {
            let status = pidfd.wait_pidfd().unwrap();
            assert!(
                matches!(status, WaitStatus::Signaled(SIGUSR1)),
                "a child killed by SIGUSR1 must report Signaled(SIGUSR1)"
            );
        }
    }
}
