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
            sys::rw::write_all(&wr, b"r").unwrap();
            let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
            let n = sys::poll::poll(&mut pfd, 5000).unwrap();
            let got = n > 0 && pfd.get(0).unwrap().revents & sys::poll::POLLIN != 0;
            // Exit 42 only if the signal actually arrived; a stub `kill` that
            // never sends would leave the poll empty and the child exits 0.
            sys::exit(if got { 42 } else { 0 });
        }
        Some(pidfd) => {
            // parent
            drop(wr);
            let mut buf = [0u8; 1];
            sys::rw::read(&rd, &mut buf).unwrap();
            sys::kill::kill(child_pid, SIGUSR1).unwrap();
            let status = sys::wait_pidfd::wait_pidfd(&pidfd).unwrap();
            assert!(
                matches!(status, WaitStatus::Exited(42)),
                "child must exit 42 after SIGUSR1"
            );
        }
    }
}
