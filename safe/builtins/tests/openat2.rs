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

    let _shellfd = sys::shellfd::reserve_shellfd().unwrap();
    let (a, b) = sys::net::socketpair().unwrap();
    a.verify().unwrap();
    b.verify().unwrap();
    a.export_to(sys::shellfd::SHELLFD).unwrap();
    a.try_close().unwrap();
    let receiver = b;
    sys::shellfd::set_capture_active(true);

    let cfg = builtins::openat2::parse::openat2_parse(&[cpath.as_c_str()]).unwrap();
    builtins::openat2::openat2_exec(&cfg).unwrap();

    let mut tag = [0u8; TAG_MAX];
    let (fd, _tag) = sys::shellfd::recv_fd(&receiver, &mut tag, std::process::id() as i32).unwrap();
    fd.verify().unwrap();

    let after = sys::stat::fstat(&fd).unwrap();
    assert_eq!(before, after);

    fd.try_close().unwrap();
    receiver.try_close().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}
