#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use sys::memfd::memfd_create;

#[test]
fn fsync_flushes_memfd() {
    let fd = memfd_create().unwrap();
    fd.write_all(b"data").unwrap();
    fd.fsync().unwrap();
    fd.lseek(0, sys::fcntl::SEEK_SET).unwrap();
    let mut buf = [0u8; 8];
    let n = fd.read(&mut buf).unwrap();
    assert_eq!(n, 4);
    assert_eq!(&buf[..n], b"data");
}

#[test]
fn fsync_after_truncate_succeeds() {
    let fd = memfd_create().unwrap();
    fd.write_all(b"longer").unwrap();
    fd.ftruncate(3).unwrap();
    fd.fsync().unwrap();
}

#[test]
fn fsync_on_invalid_fd_is_ebadf() {
    let fd = memfd_create().unwrap();
    let raw = fd.as_raw();
    drop(fd);
    // SAFETY: `raw` is the previously closed fd number; the drop above
    // guarantees no live LocalFd owns it, so wrapping it in a LocalFd never
    // double-frees. The wrapped number is invalid, so `fsync` sees EBADF.
    let ghost = unsafe { sys::LocalFd::from_raw(raw) };
    let err = ghost.fsync().unwrap_err();
    assert_eq!(err.errno(), libc::EBADF);
    let _ = ghost;
}
