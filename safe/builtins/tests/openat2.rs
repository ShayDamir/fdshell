#![cfg_attr(test, allow(clippy::unwrap_used))]

use builtins::error::BuiltinError;
use core::ffi::CStr;
use std::ffi::CString;
use sys::fcntl::O_RDONLY;
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
    let (fd, _tag) = receiver
        .recv_fd(&mut tag, sys::Pid::from_raw(std::process::id() as i32))
        .unwrap();
    fd.verify().unwrap();

    let after = fd.fstat().unwrap();
    assert_eq!(before, after);

    drop(fd);
    drop(receiver);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// An open, CLOEXEC-clear fd number that `ImportedFd::try_from` accepts.
fn numeric_fd() -> (CString, sys::ExportedFd) {
    let (rd, wr) = sys::pipe::pipe2(0).unwrap();
    rd.verify().unwrap();
    wr.verify().unwrap();
    let exported = rd.export().unwrap();
    exported.verify().unwrap();
    let s = CString::new(format!("{}", exported.as_raw())).unwrap();
    (s, exported)
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "fdshell-test-openat2-sameas-{}-{}-{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn parse_same_as_numeric() {
    let (num, _fd) = numeric_fd();
    let cpath = CString::new("x").unwrap();
    let args = [c"--same-as", num.as_c_str(), cpath.as_c_str()];
    let cfg = builtins::openat2::parse::openat2_parse(&args).unwrap();
    assert!(cfg.dirfd.is_none());
    assert_eq!(cfg.same_as.map(|f| f.as_raw()), Some(_fd.as_raw()));
}

#[test]
fn parse_same_as_rejects_non_numeric() {
    let cpath = CString::new("x").unwrap();
    let cases: &[&CStr] = &[
        c"--same-as=AT_FDCWD",
        c"--same-as", // no value
        c"--same-as=abc",
    ];
    for c in cases {
        let args = [c, cpath.as_c_str()];
        match builtins::openat2::parse::openat2_parse(&args) {
            Err(e) => assert!(
                matches!(e.current_context(), BuiltinError::InvalidArgument(_)),
                "unexpected error: {e}"
            ),
            _ => panic!("expected Err for {c:?}"),
        }
    }
}

#[test]
fn test_openat2_exec_same_as_match() {
    let dir = scratch("match");
    let a = dir.join("a");
    std::fs::write(&a, b"aaa").unwrap();
    let ca = CString::new(a.to_str().unwrap()).unwrap();
    let before = sys::stat::stat(&ca).unwrap();

    // Reference fd: a, exported so its number is CLOEXEC-clear.
    let ref_local = sys::openat2::open(&ca, O_RDONLY).unwrap();
    let ref_fd = ref_local.export().unwrap();
    let num = CString::new(format!("{}", ref_fd.as_raw())).unwrap();

    let (shell_a, shell_b) = sys::net::socketpair().unwrap();
    shell_a.verify().unwrap();
    shell_b.verify().unwrap();
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().unwrap();
    let shell_sock = shell_a.try_clone().unwrap();
    drop(shell_a);

    let args = [c"--same-as", num.as_c_str(), ca.as_c_str()];
    let cfg = builtins::openat2::parse::openat2_parse(&args).unwrap();
    builtins::openat2::openat2_exec(&cfg, &shell_sock).unwrap();

    let mut tag = [0u8; TAG_MAX];
    let (fd, _tag) = receiver
        .recv_fd(&mut tag, sys::Pid::from_raw(std::process::id() as i32))
        .unwrap();
    fd.verify().unwrap();
    assert_eq!(fd.fstat().unwrap(), before);

    drop(fd);
    drop(receiver);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_openat2_exec_same_as_mismatch_sends_nothing() {
    let dir = scratch("mismatch");
    let a = dir.join("a");
    let b = dir.join("b");
    std::fs::write(&a, b"aaa").unwrap();
    std::fs::write(&b, b"bbb").unwrap();
    let (ca, cb) = (
        CString::new(a.to_str().unwrap()).unwrap(),
        CString::new(b.to_str().unwrap()).unwrap(),
    );

    // Reference fd names `a`; the path is `b` (same device, different inode).
    let ref_local = sys::openat2::open(&ca, O_RDONLY).unwrap();
    let ref_fd = ref_local.export().unwrap();
    let num = CString::new(format!("{}", ref_fd.as_raw())).unwrap();

    let (shell_a, shell_b) = sys::net::socketpair().unwrap();
    shell_a.verify().unwrap();
    shell_b.verify().unwrap();
    let receiver = shell_b;
    sys::shellfd::set_capture_active(true);

    shell_a.export().unwrap();
    let shell_sock = shell_a.try_clone().unwrap();
    drop(shell_a);

    let args = [c"--same-as", num.as_c_str(), cb.as_c_str()];
    let cfg = builtins::openat2::parse::openat2_parse(&args).unwrap();
    let e = builtins::openat2::openat2_exec(&cfg, &shell_sock).unwrap_err();
    assert!(
        matches!(e.current_context(), BuiltinError::SameAsMismatch),
        "unexpected error: {e}"
    );

    // The opened fd was dropped before `send_fd`: nothing was sent.
    // (The socket is non-blocking, so a queued fd would have been received.)
    let mut tag = [0u8; TAG_MAX];
    let r = receiver.recv_fd(&mut tag, sys::Pid::from_raw(std::process::id() as i32));
    match r {
        Ok(_) => panic!("no fd must be sent on mismatch"),
        Err(e) => assert!(
            matches!(e.current_context(), sys::RecvFdError::Closed),
            "no fd must be sent on mismatch: {e}"
        ),
    }

    drop(receiver);
    std::fs::remove_dir_all(&dir).unwrap();
}
