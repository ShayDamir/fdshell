#![allow(clippy::unwrap_used)]

//! Pins for the `<unistd.h>` wrappers (task #614). Each wrapper is pinned
//! against an independent oracle: procfs for the uid and the hostname, direct
//! libc for `ttyname` (the crate has `libc` as a dependency, and the test
//! binary links it).

use std::ffi::{CStr, CString};

use sys::{openat2, pty, unistd};

/// Read a whole (small) file through the crate's own `openat2::open` + `read`.
fn read_file(path: &CStr) -> Vec<u8> {
    let fd = openat2::open(path, sys::fcntl::O_RDONLY).unwrap();
    let mut buf = vec![0u8; 8192];
    let n = fd.read_all(&mut buf).unwrap();
    buf.truncate(n);
    buf
}

/// Direct `libc::ttyname` as the independent oracle: NUL-trimmed bytes,
/// asserted non-null — it is only consulted on real terminals.
fn libc_ttyname(fd: i32) -> Vec<u8> {
    // SAFETY: `fd` is an open terminal fd; the returned glibc-static buffer
    // is copied immediately, before the next libc call.
    let p = unsafe { libc::ttyname(fd) };
    assert!(!p.is_null());
    // SAFETY: a non-null answer is a NUL-terminated C string in glibc's
    // static buffer.
    unsafe { CStr::from_ptr(p) }.to_bytes().to_vec()
}

#[test]
fn geteuid_matches_proc_status() {
    let data = read_file(c"/proc/self/status");
    let line = data
        .split(|b| *b == b'\n')
        .find(|l| l.starts_with(b"Uid:"))
        .unwrap();
    let s = std::str::from_utf8(line).unwrap();
    // The `Uid:` line is `real effective saved fs`; the effective field is
    // second, and uid is not 0 here (measured: plan §3), so the value pins.
    let effective: u32 = s.split_whitespace().nth(1).unwrap().parse().unwrap();
    assert_eq!(unistd::geteuid(), effective);
    // SAFETY: `geteuid()` always succeeds; this direct call is the oracle.
    assert_eq!(unistd::geteuid(), unsafe { libc::geteuid() });
}

#[test]
fn gethostname_matches_proc_oracle() {
    let mut oracle = read_file(c"/proc/sys/kernel/hostname");
    // Stated precondition (plan §3): non-empty on host and in the sandbox.
    assert!(
        !oracle.is_empty(),
        "the hostname oracle file must be non-empty"
    );
    if oracle.last() == Some(&b'\n') {
        oracle.pop();
    }
    // The buffer dwarfs the kernel's UTS name limit, so the answer is always
    // the whole name: exact equality with the procfs value.
    assert_eq!(unistd::gethostname().unwrap(), oracle);
}

#[test]
fn ttyname_agrees_with_libc_on_both_arms() {
    let (_master, slave) = pty::openpty().unwrap();
    let got = slave.ttyname().unwrap();
    // devpts is mounted in both test environments, so the shape is pinnable.
    assert!(got.starts_with(b"/dev/"), "{got:?}");
    assert_eq!(got, libc_ttyname(slave.as_raw()));

    let null_fd = openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
    assert!(null_fd.ttyname().is_err(), "/dev/null is not a terminal");
}

#[test]
fn ttyname_on_imported_fd() {
    let (_master, slave) = pty::openpty().unwrap();
    // SAFETY: `dup` on a valid fd returns a new fd or -1; the test owns it.
    let raw = unsafe { libc::dup(slave.as_raw()) };
    assert!(raw >= 0);
    // SAFETY: `F_SETFD` on a valid fd only sets the flag; the fd stays open.
    unsafe { libc::fcntl(raw, libc::F_SETFD, 0) };
    // The `ImportedFd` is built from its decimal string (STYLE §5.5).
    let name = CString::new(raw.to_string()).unwrap();
    let imported = sys::ImportedFd::try_from(name.as_c_str()).unwrap();
    assert_eq!(imported.ttyname().unwrap(), libc_ttyname(raw));

    // Under nextest the test's stdin is a pipe, so fd 0 is not a terminal:
    // `IN` is a real-data negative arm of the `ImportedFd` impl.
    assert!(sys::IN.ttyname().is_err());

    // SAFETY: `raw` is the test's own fd, still open.
    unsafe { libc::close(raw) };
}
