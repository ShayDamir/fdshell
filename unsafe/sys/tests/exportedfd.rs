#![allow(clippy::unwrap_used, clippy::expect_used)]

use sys::{ExportedFd, ImportedFd};

#[test]
fn display_formats_raw() {
    let local = sys::openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
    let exported = local.export().unwrap();
    let formatted = format!("{}", exported);
    assert_eq!(formatted, exported.as_raw().to_string());
}

#[test]
fn verify_accepts_non_cloexec() {
    let local = sys::openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
    let exported = local.export().unwrap();
    assert!(exported.verify().is_ok());
}

#[test]
fn verify_rejects_cloexec() {
    // SAFETY: `/dev/null` is a valid path; O_RDONLY|O_CLOEXEC are valid flags.
    let fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    assert!(fd >= 0);
    // SAFETY: `fd` is a valid open fd — we intentionally opened with CLOEXEC
    // to verify that ExportedFd::verify() catches this.
    let cloexec = unsafe { ExportedFd::from_raw(fd) };
    assert!(cloexec.verify().is_err());
    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fd) };
}

#[test]
fn as_raw_returns_valid_fd() {
    let local = sys::openat2::open(c"/dev/null", sys::fcntl::O_RDONLY).unwrap();
    let exported = local.export().unwrap();
    let raw = exported.as_raw();
    // Mutants return -1/0/1. Only real fd is >= 2 (dup'ed from /dev/null).
    assert!(raw >= 2);
    // Verify the returned fd number is actually usable by reading from it.
    let mut buf = [0u8; 1];
    // SAFETY: `raw` is a valid open fd without CLOEXEC (dup strips it).
    let imported = unsafe { ImportedFd::from_raw(raw) };
    let n = imported.read(&mut buf).unwrap();
    assert_eq!(n, 0); // /dev/null returns 0 bytes
}
