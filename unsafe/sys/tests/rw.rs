#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use sys::pipe::pipe2;
use sys::rw::{read, write};

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
