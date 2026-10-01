#![allow(clippy::unwrap_used)]

use crate::error::BuiltinError;
use crate::strict::{require_dirfd, require_relative};

/// An explicit (non-`AT_FDCWD`) dirfd: a live pipe end parsed to an
/// [`ImportedFd`]. The pipe is returned so the caller holds it for the test's
/// duration (the guard only inspects `is_none()`, never the fd itself).
fn explicit_dirfd() -> (sys::LocalFd, sys::ImportedFd) {
    let (rd, _wr) = sys::pipe::pipe2(0).unwrap();
    rd.verify().unwrap();
    let s = alloc::format!("{}", rd.export().unwrap().as_raw());
    (rd, sys::ImportedFd::from_bytes(s.as_bytes()).unwrap())
}

#[test]
fn dirfd_bans_ateq_when_strict() {
    assert!(matches!(
        require_dirfd(true, None).unwrap_err().current_context(),
        BuiltinError::StrictRequiresDirfd
    ));
}

#[test]
fn dirfd_allows_explicit_when_strict() {
    let (_keep, fd) = explicit_dirfd();
    require_dirfd(true, Some(&fd)).unwrap();
}

#[test]
fn dirfd_allows_ateq_when_not_strict() {
    require_dirfd(false, None).unwrap();
}

#[test]
fn path_bans_absolute_when_strict() {
    assert!(matches!(
        require_relative(true, c"/etc/passwd")
            .unwrap_err()
            .current_context(),
        BuiltinError::StrictAbsolutePath
    ));
}

#[test]
fn path_allows_relative_when_strict() {
    require_relative(true, c"sub/file").unwrap();
}

#[test]
fn path_allows_absolute_when_not_strict() {
    require_relative(false, c"/etc/passwd").unwrap();
}
