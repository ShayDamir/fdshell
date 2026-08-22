#![allow(clippy::expect_used, clippy::unwrap_used)]

use sys::pipe::pipe2;

#[test]
fn closing_an_open_fd_succeeds() {
    let (rd, _wr) = pipe2(0).unwrap();
    let n = rd.as_raw();
    sys::close::close(n).unwrap();
}

#[test]
fn closing_a_closed_fd_fails() {
    let (rd, _wr) = pipe2(0).unwrap();
    let n = rd.as_raw();
    drop(rd);
    let result = sys::close::close(n);
    assert!(matches!(
        result,
        Err(ref e) if e.errno() == libc::EBADF
    ));
}
