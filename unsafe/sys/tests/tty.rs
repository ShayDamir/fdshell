#![allow(clippy::unwrap_used, clippy::expect_used)]

use sys::pty::openpty;
use sys::tty::isatty;

/// The pty slave is a terminal device.
#[test]
fn isatty_true_on_pty() {
    let (_master, slave) = openpty().unwrap();
    assert!(isatty(&slave), "pty slave must be a tty");
}

/// A character device that is not a terminal (/dev/null) is not a tty.
#[test]
fn isatty_false_on_dev_null() {
    let fd = sys::openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
    assert!(!isatty(&fd), "/dev/null is not a tty");
}

/// `openpty` must set CLOEXEC on both ends (the `LocalFd` invariant).
#[test]
fn openpty_sets_cloexec() {
    let (master, slave) = openpty().unwrap();
    master.verify().expect("master must have CLOEXEC");
    slave.verify().expect("slave must have CLOEXEC");
}
