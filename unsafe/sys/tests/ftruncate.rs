#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use sys::memfd::memfd_create;
use sys::rw::{lseek, read, write_all};

#[test]
fn ftruncate_shrinks_and_reads_prefix() {
    let fd = memfd_create().unwrap();
    write_all(&fd, b"hello").unwrap();
    sys::ftruncate::ftruncate(&fd, 2).unwrap();
    lseek(&fd, 0, sys::fcntl::SEEK_SET).unwrap();
    let mut buf = [0u8; 8];
    let n = read(&fd, &mut buf).unwrap();
    assert_eq!(n, 2);
    assert_eq!(&buf[..n], b"he");
    // EOF right after the truncated size.
    assert_eq!(read(&fd, &mut buf).unwrap(), 0);
}

#[test]
fn ftruncate_extends_with_zeroes() {
    let fd = memfd_create().unwrap();
    write_all(&fd, b"abc").unwrap();
    sys::ftruncate::ftruncate(&fd, 6).unwrap();
    lseek(&fd, 0, sys::fcntl::SEEK_SET).unwrap();
    let mut buf = [0u8; 8];
    let n = read(&fd, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"abc\0\0\0");
}

#[test]
fn ftruncate_extends_then_shrinks() {
    let fd = memfd_create().unwrap();
    write_all(&fd, b"xy").unwrap();
    sys::ftruncate::ftruncate(&fd, 4).unwrap();
    sys::ftruncate::ftruncate(&fd, 1).unwrap();
    lseek(&fd, 0, sys::fcntl::SEEK_SET).unwrap();
    let mut buf = [0u8; 8];
    let n = read(&fd, &mut buf).unwrap();
    assert_eq!(n, 1);
    assert_eq!(&buf[..n], b"x");
}

#[test]
fn ftruncate_zero_length_empties_file() {
    let fd = memfd_create().unwrap();
    write_all(&fd, b"hello").unwrap();
    sys::ftruncate::ftruncate(&fd, 0).unwrap();
    let mut buf = [0u8; 8];
    assert_eq!(read(&fd, &mut buf).unwrap(), 0);
}

#[test]
fn ftruncate_on_pipe_is_einval() {
    let (rd, wr) = sys::pipe::pipe2(0).unwrap();
    let err = sys::ftruncate::ftruncate(&rd, 4).unwrap_err();
    assert_eq!(err.errno(), libc::EINVAL);
    let _ = wr;
}

#[test]
fn ftruncate_negative_length_is_einval() {
    let fd = memfd_create().unwrap();
    let err = sys::ftruncate::ftruncate(&fd, -1).unwrap_err();
    assert_eq!(err.errno(), libc::EINVAL);
}
