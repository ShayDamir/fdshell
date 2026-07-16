#![allow(clippy::unwrap_used, clippy::expect_used)]

use sys::pipe::pipe2;

/// pipe2 must always produce fds with CLOEXEC set, satisfying the LocalFd invariant.
#[test]
fn test_pipe2_with_cloexec() {
    let (rd, wr) = pipe2(0).unwrap();
    rd.verify().expect("read end must have CLOEXEC");
    wr.verify().expect("write end must have CLOEXEC");
}
