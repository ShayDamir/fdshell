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

    // Create a socket to use as the shell fd
    let (shell_a, shell_b) = sys::net::socketpair().unwrap();
    shell_a.verify().unwrap();
    shell_b.verify().unwrap();
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().unwrap();
    let shell_sock = shell_a.try_clone().unwrap();
    drop(shell_a);

    let cfg = builtins::openat2::parse::openat2_parse(&[cpath.as_c_str()]).unwrap();
    builtins::openat2::openat2_exec(&cfg, &shell_sock).unwrap();

    let mut tag = [0u8; TAG_MAX];
    let (fd, _tag) = sys::shellfd::recv_fd(&receiver, &mut tag, std::process::id() as i32).unwrap();
    fd.verify().unwrap();

    let after = sys::stat::fstat(&fd).unwrap();
    assert_eq!(before, after);

    drop(fd);
    drop(receiver);
    std::fs::remove_dir_all(&dir).unwrap();
}
