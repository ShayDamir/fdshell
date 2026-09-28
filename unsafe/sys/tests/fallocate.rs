#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use sys::memfd::memfd_create;

#[test]
fn fallocate_grows_empty_file() {
    let fd = memfd_create().unwrap();
    fd.fallocate(0, 0, 4096).unwrap();
    assert_eq!(fd.fstat().unwrap().size, 4096);
}

#[test]
fn fallocate_extends_past_current_size() {
    let fd = memfd_create().unwrap();
    fd.write_all(b"ab").unwrap();
    fd.fallocate(0, 0, 8192).unwrap();
    assert_eq!(fd.fstat().unwrap().size, 8192);
}

#[test]
fn fallocate_within_current_size_is_noop() {
    let fd = memfd_create().unwrap();
    fd.fallocate(0, 0, 4096).unwrap();
    fd.fallocate(0, 100, 16).unwrap();
    assert_eq!(fd.fstat().unwrap().size, 4096);
}

#[test]
fn fallocate_zero_len_is_einval() {
    let fd = memfd_create().unwrap();
    let err = fd.fallocate(0, 0, 0).unwrap_err();
    assert_eq!(err.errno(), libc::EINVAL);
}

#[test]
fn fallocate_negative_len_is_einval() {
    let fd = memfd_create().unwrap();
    let err = fd.fallocate(0, 0, -1).unwrap_err();
    assert_eq!(err.errno(), libc::EINVAL);
}

#[test]
fn fallocate_on_pipe_is_ebadf() {
    let (rd, _wr) = sys::pipe::pipe2(0).unwrap();
    let err = rd.fallocate(0, 0, 4096).unwrap_err();
    assert_eq!(err.errno(), libc::EBADF);
}
