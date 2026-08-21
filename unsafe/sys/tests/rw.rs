#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use sys::pipe::pipe2;
use sys::rw::{lseek, read, write, write_all};

#[test]
fn read_returns_exact_count() {
    let (rd, wr) = pipe2(0).unwrap();
    let data = b"0123456789"; // 10 bytes
    write(&wr, data).unwrap();
    let mut buf = [0u8; 64];
    let n = read(&rd, &mut buf).unwrap();
    assert_eq!(n, 10);
    assert_eq!(&buf[..n], data);
}

#[test]
fn write_returns_exact_count() {
    let (rd, wr) = pipe2(0).unwrap();
    let data = b"0123456789"; // 10 bytes
    let n = write(&wr, data).unwrap();
    assert_eq!(n, 10);
    let mut buf = [0u8; 64];
    let _ = read(&rd, &mut buf).unwrap();
}

#[test]
fn write_all_writes_full_buffer() {
    let fd = sys::memfd::memfd_create().unwrap();
    let data: Vec<u8> = (0u8..200).collect();
    write_all(&fd, &data).unwrap();
    lseek(&fd, 0, sys::fcntl::SEEK_SET).unwrap();
    let mut buf = [0u8; 256];
    let n = read(&fd, &mut buf).unwrap();
    assert_eq!(n, 200);
    assert_eq!(&buf[..n], &data[..]);
}

#[test]
fn write_all_empty_buffer_is_noop() {
    let fd = sys::memfd::memfd_create().unwrap();
    write_all(&fd, b"").unwrap();
    let mut buf = [0u8; 4];
    assert_eq!(read(&fd, &mut buf).unwrap(), 0);
}

#[test]
fn lseek_moves_offset_and_returns_new_position() {
    let fd = sys::memfd::memfd_create().unwrap();
    write_all(&fd, b"abcdef").unwrap();
    let pos = lseek(&fd, 3, sys::fcntl::SEEK_SET).unwrap();
    assert_eq!(pos, 3);
    let mut buf = [0u8; 3];
    let n = read(&fd, &mut buf).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf[..n], b"def");
}

#[test]
fn lseek_back_to_start_rereads_content() {
    let fd = sys::memfd::memfd_create().unwrap();
    write_all(&fd, b"abcdef").unwrap();
    let pos = lseek(&fd, 0, sys::fcntl::SEEK_SET).unwrap();
    assert_eq!(pos, 0);
    let mut buf = [0u8; 3];
    assert_eq!(read(&fd, &mut buf).unwrap(), 3);
    assert_eq!(&buf[..3], b"abc");
}

#[test]
fn cvt64_passes_through_and_maps_minus_one_to_error() {
    assert_eq!(sys::cvt64(123), Ok(123));
    assert!(sys::cvt64(-1).is_err());
}
