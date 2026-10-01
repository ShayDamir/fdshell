#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use alloc::ffi::CString;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use builtins::error::BuiltinError;

use crate::child::Ctx;
use crate::state::ShellState;
use std::io::Read;

use super::handle_connect;

fn name(tag: &str) -> String {
    format!("fdshell-connect-{tag}-{}", std::process::id())
}

fn cstr(s: &str) -> CString {
    CString::new(s).unwrap()
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

/// Read exactly `buf.len()` bytes from `fd` into `buf`.
fn read_exact(fd: &sys::LocalFd, buf: &mut [u8]) {
    let mut got = 0;
    while got < buf.len() {
        let n = fd.read(&mut buf[got..]).unwrap();
        assert!(n > 0, "fd must stay readable");
        got += n;
    }
}

#[test]
fn connect_handler_sends_connected_abstract_socket() {
    let n = name("abs");
    // The listener is built by the test (as `listen/tests.rs` does); the
    // handler's socket must connect to it.
    let listener = sys::net::socket(sys::net::AF_UNIX, sys::net::SOCK_STREAM).unwrap();
    sys::net::bind_uds_abstract(&listener, cstr(&n)).unwrap();
    sys::net::listen(&listener, 1).unwrap();
    let (state, receiver) = capture_state();
    with_refs(&[&format!("@{n}")], |refs| {
        assert_eq!(
            handle_connect(&Ctx::new(c"connect".into(), refs, &[], &state)).unwrap(),
            0
        );
    });
    let (fd, tag) = recv(&receiver);
    assert_eq!(tag, b"connect");
    let conn = sys::net::accept(&listener).unwrap();
    // A byte round-trip proves the captured fd is connected, not just open.
    assert_eq!(fd.write(b"hi\n").unwrap(), 3);
    let mut buf = [0u8; 3];
    read_exact(&conn, &mut buf);
    assert_eq!(&buf, b"hi\n");
}

#[test]
fn connect_handler_dgram_sends_connected_socket() {
    let n = name("dgram");
    // A bound dgram socket is the peer (no `listen` for datagrams).
    let listener = sys::net::socket(sys::net::AF_UNIX, sys::net::SOCK_DGRAM).unwrap();
    sys::net::bind_uds_abstract(&listener, cstr(&n)).unwrap();
    let (state, receiver) = capture_state();
    with_refs(&["--type", "dgram", &format!("@{n}")], |refs| {
        assert_eq!(
            handle_connect(&Ctx::new(c"connect".into(), refs, &[], &state)).unwrap(),
            0
        );
    });
    let (fd, tag) = recv(&receiver);
    assert_eq!(tag, b"connect");
    // `--type dgram` really built a datagram socket: the write lands on the
    // bound peer.
    assert_eq!(fd.write(b"ping\n").unwrap(), 5);
    let mut buf = [0u8; 5];
    read_exact(&listener, &mut buf);
    assert_eq!(&buf, b"ping\n");
}

#[test]
fn connect_handler_path_sends_connected_socket() {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir()
        .join(name("path"))
        .with_extension(format!("{c}"));
    // The listener (and its socket file) is built by the test: `connect`
    // never creates a filesystem object.
    let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    let (state, receiver) = capture_state();
    let path_c = path.to_string_lossy().into_owned();
    with_refs(&[&path_c], |refs| {
        assert_eq!(
            handle_connect(&Ctx::new(c"connect".into(), refs, &[], &state)).unwrap(),
            0
        );
    });
    let (fd, tag) = recv(&receiver);
    assert_eq!(tag, b"connect");
    let (mut conn, _) = listener.accept().unwrap();
    assert_eq!(fd.write(b"hi\n").unwrap(), 3);
    let mut buf = [0u8; 3];
    let mut got = 0;
    while got < buf.len() {
        let n = conn.read(&mut buf[got..]).unwrap();
        assert!(n > 0, "accepted fd must stay readable");
        got += n;
    }
    assert_eq!(&buf, b"hi\n");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn connect_handler_inet_sends_connected_socket() {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let (state, receiver) = capture_state();
    with_refs(
        &["--bind", "127.0.0.1", "--port", &format!("{port}")],
        |refs| {
            assert_eq!(
                handle_connect(&Ctx::new(c"connect".into(), refs, &[], &state)).unwrap(),
                0
            );
        },
    );
    let (fd, tag) = recv(&receiver);
    assert_eq!(tag, b"connect");
    let (mut conn, _) = listener.accept().unwrap();
    assert_eq!(fd.write(b"hi\n").unwrap(), 3);
    let mut buf = [0u8; 3];
    let mut got = 0;
    while got < buf.len() {
        let n = conn.read(&mut buf[got..]).unwrap();
        assert!(n > 0, "accepted fd must stay readable");
        got += n;
    }
    assert_eq!(&buf, b"hi\n");
}

#[test]
fn connect_handler_connect_refused_is_syscall_error() {
    let (state, _receiver) = capture_state();
    let n = name("refused");
    with_refs(&[&format!("@{n}")], |refs| {
        let e = handle_connect(&Ctx::new(c"connect".into(), refs, &[], &state)).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Syscall));
    });
}

#[test]
fn connect_handler_without_capture_socket_fails() {
    with_refs(&["@whatever"], |refs| {
        let e = handle_connect(&Ctx::new(c"connect".into(), refs, &[], &ShellState::new()))
            .unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::SendFdFailed));
    });
}

#[test]
fn connect_handler_missing_address_is_missing_argument() {
    with_refs(&[], |refs| {
        let e = handle_connect(&Ctx::new(c"connect".into(), refs, &[], &ShellState::new()))
            .unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::MissingArgument("address")
        ));
    });
}
