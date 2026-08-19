#![allow(clippy::unwrap_used)]

use sys::pidfd_send_signal::{SIGKILL, send_signal};
use sys::pipe::pipe2;

#[test]
fn send_signal_non_pidfd_errors() {
    let (rd, _wr) = pipe2(0).unwrap();
    // A pipe end is not a pidfd, so the syscall must fail — a stub returning
    // Ok(()) would hide the EBADF the kernel returns for a non-pidfd.
    assert!(send_signal(&rd, SIGKILL).is_err());
}
