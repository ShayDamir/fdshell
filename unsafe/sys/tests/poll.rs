#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use sys::pipe::pipe2;
use sys::poll::{POLLIN, PollFd, poll};

/// A descriptor with pending data must be reported ready with `POLLIN` set.
#[test]
fn poll_reports_ready_descriptor() {
    let (rd, wr) = pipe2(0).unwrap();
    sys::rw::write(&wr, b"x").unwrap();
    let mut fds = [PollFd::new(rd.as_raw(), POLLIN)];
    let n = poll(&mut fds, 0).unwrap();
    assert_eq!(n, 1);
    assert_ne!(fds[0].revents & POLLIN, 0, "ready fd must report POLLIN");
}

/// An idle descriptor (no data, write end open) with a short timeout returns 0.
#[test]
fn poll_times_out_on_idle_descriptor() {
    let (rd, _wr) = pipe2(0).unwrap();
    let mut fds = [PollFd::new(rd.as_raw(), POLLIN)];
    let n = poll(&mut fds, 1).unwrap();
    assert_eq!(n, 0);
}
