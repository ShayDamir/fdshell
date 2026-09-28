#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use builtins::error::BuiltinError;

use crate::child::Ctx;
use crate::state::ShellState;
use std::os::unix::fs::FileTypeExt;

use super::handle_bind;

fn name(tag: &str) -> String {
    format!("fdshell-bind-{tag}-{}", std::process::id())
}

fn with_refs<R, F>(args: &[&str], f: F) -> R
where
    F: FnOnce(&[&core::ffi::CStr]) -> R,
{
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    f(&refs)
}

/// A socketpair plays the parent's capture socket; the received end plays
/// the parent's `do_captures`.
fn capture_state() -> (ShellState, sys::LocalFd) {
    let (shell_sock, receiver) = sys::net::socketpair().unwrap();
    shell_sock.verify().unwrap();
    receiver.verify().unwrap();
    sys::shellfd::set_capture_active(true);
    let mut state = ShellState::new();
    state.set_shell_sock(shell_sock);
    (state, receiver)
}

fn recv(receiver: &sys::LocalFd) -> (sys::LocalFd, Vec<u8>) {
    let mut buf = [0u8; sys::shellfd::TAG_MAX];
    let pid = sys::Pid::from_raw(std::process::id() as i32);
    let (fd, tag) = receiver.recv_fd(&mut buf, pid).unwrap();
    fd.verify().unwrap();
    (fd, tag.to_bytes().to_vec())
}

#[test]
fn bind_handler_sends_abstract_socket() {
    let (state, receiver) = capture_state();
    let n = name("abs");
    with_refs(&[&format!("@{n}")], |refs| {
        assert_eq!(
            handle_bind(&Ctx::new(c"bind".into(), refs, &[], &state)).unwrap(),
            0
        );
    });
    let (fd, tag) = recv(&receiver);
    assert_eq!(tag, b"bind");
    drop(fd);
}

#[test]
fn bind_handler_sends_dgram_socket() {
    let (state, receiver) = capture_state();
    let n = name("dgram");
    with_refs(&["--type", "dgram", &format!("@{n}")], |refs| {
        assert_eq!(
            handle_bind(&Ctx::new(c"bind".into(), refs, &[], &state)).unwrap(),
            0
        );
    });
    let (fd, tag) = recv(&receiver);
    assert_eq!(tag, b"bind");
    drop(fd);
}

#[test]
fn bind_handler_sends_inet_socket() {
    let (state, receiver) = capture_state();
    with_refs(&["--bind", "127.0.0.1", "--port", "0"], |refs| {
        assert_eq!(
            handle_bind(&Ctx::new(c"bind".into(), refs, &[], &state)).unwrap(),
            0
        );
    });
    let (fd, tag) = recv(&receiver);
    assert_eq!(tag, b"bind");
    drop(fd);
}

#[test]
fn bind_handler_path_creates_socket_file() {
    let (state, receiver) = capture_state();
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir()
        .join(name("path"))
        .with_extension(format!("{c}"));
    let path_c = path.to_string_lossy().into_owned();
    with_refs(&[&path_c], |refs| {
        assert_eq!(
            handle_bind(&Ctx::new(c"bind".into(), refs, &[], &state)).unwrap(),
            0
        );
    });
    let (fd, tag) = recv(&receiver);
    assert_eq!(tag, b"bind");
    drop(fd);
    let meta = std::fs::metadata(&path).unwrap();
    assert!(meta.file_type().is_socket());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn bind_handler_duplicate_abstract_name_is_syscall_error() {
    let (state, receiver) = capture_state();
    let n = name("dup");
    // First bind succeeds; hold the socket open (dropping it would release
    // the abstract name) so the second bind hits EADDRINUSE.
    let held = with_refs(&[&format!("@{n}")], |refs| {
        assert_eq!(
            handle_bind(&Ctx::new(c"bind".into(), refs, &[], &state)).unwrap(),
            0
        );
        recv(&receiver).0
    });
    with_refs(&[&format!("@{n}")], |refs| {
        let e = handle_bind(&Ctx::new(c"bind".into(), refs, &[], &state)).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Syscall));
    });
    drop(held);
}

#[test]
fn bind_handler_without_capture_socket_fails() {
    with_refs(&["@whatever"], |refs| {
        let e = handle_bind(&Ctx::new(c"bind".into(), refs, &[], &ShellState::new())).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::SendFdFailed));
    });
}

#[test]
fn bind_handler_missing_address_is_missing_argument() {
    with_refs(&[], |refs| {
        let e = handle_bind(&Ctx::new(c"bind".into(), refs, &[], &ShellState::new())).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::MissingArgument("address")
        ));
    });
}

#[test]
fn bind_handler_bad_type_is_invalid_argument() {
    with_refs(&["--type", "bogus", "x"], |refs| {
        let e = handle_bind(&Ctx::new(c"bind".into(), refs, &[], &ShellState::new())).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::InvalidArgument("type")
        ));
    });
}
