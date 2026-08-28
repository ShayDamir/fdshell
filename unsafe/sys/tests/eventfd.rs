#![cfg_attr(test, allow(clippy::unwrap_used))]

use sys::eventfd::{self, EFD_CLOEXEC, EFD_NONBLOCK};

#[test]
fn create_sets_cloexec() {
    let fd = eventfd::eventfd(0, 0).unwrap();
    // A stub that drops the `| EFD_CLOEXEC` would fail this check.
    fd.verify().unwrap();
}

#[test]
fn create_keeps_cloexec_when_requested() {
    let fd = eventfd::eventfd(0, EFD_CLOEXEC).unwrap();
    // A `^` instead of `|` would clear CLOEXEC when it is already set.
    fd.verify().unwrap();
}

#[test]
fn create_accepts_nonblock() {
    let fd = eventfd::eventfd(0, EFD_NONBLOCK).unwrap();
    fd.verify().unwrap();
}

#[test]
fn init_nonzero_is_readable() {
    let fd = eventfd::eventfd(1, 0).unwrap();
    // A non-zero initial counter makes the fd readable immediately.
    let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
    let n = sys::poll::poll(&mut pfd, 2000).unwrap();
    assert_eq!(n, 1);
    let revents = pfd.get(0).unwrap().revents;
    assert_ne!(revents & sys::poll::POLLIN, 0);
}

#[test]
fn init_zero_is_not_readable() {
    let fd = eventfd::eventfd(0, 0).unwrap();
    // A zero counter is not readable until written.
    let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
    let n = sys::poll::poll(&mut pfd, 100).unwrap();
    assert_eq!(n, 0, "a zero-counter eventfd must not be readable");
}

#[test]
fn write_makes_readable() {
    let fd = eventfd::eventfd(0, 0).unwrap();
    // Writing an 8-byte counter increments it, making the fd readable.
    sys::rw::write(&fd, &1u64.to_ne_bytes()).unwrap();
    let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
    let n = sys::poll::poll(&mut pfd, 2000).unwrap();
    assert_eq!(n, 1);
    let revents = pfd.get(0).unwrap().revents;
    assert_ne!(revents & sys::poll::POLLIN, 0);
}
