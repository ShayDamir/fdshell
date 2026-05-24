#![cfg_attr(test, allow(clippy::unwrap_used))]

use sys::shellfd::TAG_MAX;

#[test]
fn test_openat2_exec() {
    let dir = std::env::temp_dir().join("fdshell-test-openat2-exec");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("testfile");
    std::fs::write(&file_path, b"hello\n").unwrap();

    let cpath = std::ffi::CString::new(file_path.to_str().unwrap()).unwrap();
    let before = sys::stat::stat(&cpath).unwrap();

    let mut pair = [0; 2];
    sys::net::socketpair(&mut pair).unwrap();
    if pair[0] != 3 {
        sys::fd::dup2(pair[0], 3).unwrap();
        let _ = sys::fd::close(pair[0]);
    }
    let receiver = pair[1];

    let cfg = builtins::openat2::parse::openat2_parse(&[cpath.as_c_str()]).unwrap();
    builtins::openat2::openat2_exec(&cfg).unwrap();

    let mut tag = [0u8; TAG_MAX];
    let (fd, _tag) = sys::shellfd::recv_fd(receiver, &mut tag).unwrap();

    let after = sys::stat::fstat(fd).unwrap();
    assert_eq!(before, after);

    sys::fd::close(fd).unwrap();
    sys::fd::close(receiver).unwrap();
    if pair[0] != 3 {
        sys::fd::close(3).unwrap();
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
