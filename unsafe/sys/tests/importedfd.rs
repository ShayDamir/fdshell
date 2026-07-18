#![allow(clippy::unwrap_used, clippy::expect_used)]

use sys::{ImportedFd, ImportedFdError, ShortCStr};

#[test]
fn verify_internal_fd_rejected() {
    // Open /dev/null with O_CLOEXEC — CLOEXEC is set, so verify() must reject it.
    // SAFETY: `/dev/null` is a valid path; O_RDONLY|O_CLOEXEC are valid flags.
    let fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    assert!(fd >= 0);
    // SAFETY: `fd` is a valid open fd with CLOEXEC clear — wait, it has CLOEXEC set,
    // so from_raw invariant is violated. But we're testing that verify() catches this.
    let d = unsafe { ImportedFd::from_raw(fd) };
    let result = d.verify();
    assert!(matches!(
        result,
        Err(ref e) if matches!(e.current_context(), ImportedFdError::InternalFd)
    ));
    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fd) };
}

#[test]
fn from_shortcstr_stdin() {
    let short = ShortCStr::from_vec(b"0".to_vec()).expect("test");
    let fd = ImportedFd::from_shortcstr(&short).expect("test");
    assert_eq!(fd.as_raw(), 0);
}

#[test]
fn from_shortcstr_invalid_number() {
    let short = ShortCStr::from_vec(b"notanumber".to_vec()).expect("test");
    let result = ImportedFd::from_shortcstr(&short);
    assert!(matches!(
        result,
        Err(ref e) if matches!(e.current_context(), ImportedFdError::NotANumber)
    ));
}

#[test]
fn from_shortcstr_negative() {
    let short = ShortCStr::from_vec(b"-1".to_vec()).expect("test");
    let result = ImportedFd::from_shortcstr(&short);
    assert!(matches!(
        result,
        Err(ref e) if matches!(e.current_context(), ImportedFdError::Negative)
    ));
}

#[test]
fn from_shortcstr_internal_fd_rejected() {
    // Open /dev/null with O_CLOEXEC — CLOEXEC is set, so verify() must reject it.
    // SAFETY: `/dev/null` is a valid path; O_RDONLY|O_CLOEXEC are valid flags.
    let fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    assert!(fd >= 0);
    let num = format!("{}", fd);
    let short = ShortCStr::from_vec(num.into_bytes()).expect("test");
    let result = ImportedFd::from_shortcstr(&short);
    assert!(matches!(
        result,
        Err(ref e) if matches!(e.current_context(), ImportedFdError::InternalFd)
    ));
    // SAFETY: `fd` is a valid open fd from the test above.
    unsafe { libc::close(fd) };
}
