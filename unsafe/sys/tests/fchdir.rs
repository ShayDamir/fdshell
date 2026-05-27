#![allow(clippy::unwrap_used)]

#[test]
fn fchdir_ok() {
    let cwd = std::env::current_dir().unwrap();
    // SAFETY: opening "." with O_RDONLY|O_CLOEXEC is always valid.
    let raw = unsafe { libc::open(c".".as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    assert!(raw >= 0, "open CWD failed");

    // SAFETY: `raw` is a valid fd with CLOEXEC, guaranteed by the open flags above.
    let fd = unsafe { sys::Fd::from_raw(raw) };
    sys::fchdir::fchdir(&fd).unwrap();

    // fchdir to CWD should leave CWD unchanged.
    assert_eq!(std::env::current_dir().unwrap(), cwd);
    fd.close().unwrap();
}

#[test]
fn fchdir_ebadf() {
    // SAFETY: -1 is never a valid fd; fchdir returns EBADF.
    let fd = unsafe { sys::Fd::from_raw(-1) };
    let err = sys::fchdir::fchdir(&fd).unwrap_err();
    assert_eq!(err, libc::EBADF);
}
