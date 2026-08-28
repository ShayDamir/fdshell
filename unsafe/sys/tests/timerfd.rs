#![cfg_attr(test, allow(clippy::unwrap_used))]

use sys::timerfd::{self, TFD_CLOEXEC, TFD_NONBLOCK};

#[test]
fn create_sets_cloexec() {
    let fd = timerfd::timerfd_create(0).unwrap();
    // A stub that drops the `| TFD_CLOEXEC` would fail this check.
    fd.verify().unwrap();
}

#[test]
fn create_keeps_cloexec_when_requested() {
    let fd = timerfd::timerfd_create(TFD_CLOEXEC).unwrap();
    // A `^` instead of `|` would clear CLOEXEC when it is already set.
    fd.verify().unwrap();
}

#[test]
fn create_accepts_nonblock() {
    let fd = timerfd::timerfd_create(TFD_NONBLOCK).unwrap();
    fd.verify().unwrap();
}

#[test]
fn settime_arms_the_timer() {
    let fd = timerfd::timerfd_create(0).unwrap();
    // A stub returning `Ok(())` without arming would leave the fd unreadable.
    timerfd::timerfd_settime(&fd, (0, 10_000_000), (0, 0)).unwrap();
    let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
    let n = sys::poll::poll(&mut pfd, 2000).unwrap();
    assert_eq!(n, 1);
    let revents = pfd.first().unwrap().revents;
    assert_ne!(revents & sys::poll::POLLIN, 0);
}

#[test]
fn settime_rejects_bad_nsecs() {
    let fd = timerfd::timerfd_create(0).unwrap();
    // `tv_nsec >= 1e9` is out of range for the kernel.
    let err = timerfd::timerfd_settime(&fd, (0, 1_000_000_000), (0, 0)).unwrap_err();
    assert!(matches!(err, sys::SyscallError::EINVAL(_)), "got {err}");
}
