#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use sys::memfd::memfd_create;

#[test]
fn ftruncate_shrinks_and_reads_prefix() {
    let fd = memfd_create().unwrap();
    fd.write_all(b"hello").unwrap();
    fd.ftruncate(2).unwrap();
    fd.lseek(0, sys::fcntl::SEEK_SET).unwrap();
    let mut buf = [0u8; 8];
    let n = fd.read(&mut buf).unwrap();
    assert_eq!(n, 2);
    assert_eq!(&buf[..n], b"he");
    // EOF right after the truncated size.
    assert_eq!(fd.read(&mut buf).unwrap(), 0);
}

#[test]
fn ftruncate_extends_with_zeroes() {
    let fd = memfd_create().unwrap();
    fd.write_all(b"abc").unwrap();
    fd.ftruncate(6).unwrap();
    fd.lseek(0, sys::fcntl::SEEK_SET).unwrap();
    let mut buf = [0u8; 8];
    let n = fd.read(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"abc\0\0\0");
}

#[test]
fn ftruncate_extends_then_shrinks() {
    let fd = memfd_create().unwrap();
    fd.write_all(b"xy").unwrap();
    fd.ftruncate(4).unwrap();
    fd.ftruncate(1).unwrap();
    fd.lseek(0, sys::fcntl::SEEK_SET).unwrap();
    let mut buf = [0u8; 8];
    let n = fd.read(&mut buf).unwrap();
    assert_eq!(n, 1);
    assert_eq!(&buf[..n], b"x");
}

#[test]
fn ftruncate_zero_length_empties_file() {
    let fd = memfd_create().unwrap();
    fd.write_all(b"hello").unwrap();
    fd.ftruncate(0).unwrap();
    let mut buf = [0u8; 8];
    assert_eq!(fd.read(&mut buf).unwrap(), 0);
}

#[test]
fn ftruncate_on_pipe_is_einval() {
    let (rd, wr) = sys::pipe::pipe2(0).unwrap();
    let err = rd.ftruncate(4).unwrap_err();
    assert_eq!(err.errno(), libc::EINVAL);
    let _ = wr;
}

#[test]
fn ftruncate_negative_length_is_einval() {
    let fd = memfd_create().unwrap();
    let err = fd.ftruncate(-1).unwrap_err();
    assert_eq!(err.errno(), libc::EINVAL);
}
