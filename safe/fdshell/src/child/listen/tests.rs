#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use builtins::error::BuiltinError;

use crate::child::Ctx;
use crate::state::ShellState;

use super::handle_listen;

fn name(tag: &str) -> String {
    format!("fdshell-listen-{tag}-{}", std::process::id())
}

fn with_refs<R, F>(args: &[&str], f: F) -> R
where
    F: FnOnce(&[&core::ffi::CStr]) -> R,
{
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    f(&refs)
}

/// A socketpair plays the parent's capture socket; the receiver end stands
/// in for the parent's `do_captures`.
fn capture_state() -> (ShellState, sys::LocalFd) {
    let (shell_sock, receiver) = sys::net::socketpair().unwrap();
    shell_sock.verify().unwrap();
    receiver.verify().unwrap();
    sys::shellfd::set_capture_active(true);
    let mut state = ShellState::new();
    state.set_shell_sock(shell_sock);
    (state, receiver)
}

/// Connect a test-side client to the abstract socket bound at `name`.
fn connect_abstract(name: &str) -> std::os::unix::net::UnixStream {
    use std::os::linux::net::SocketAddrExt;
    let addr = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()).unwrap();
    std::os::unix::net::UnixStream::connect_addr(&addr).unwrap()
}

#[test]
fn listen_handler_sends_listening_socket() {
    let (state, receiver) = capture_state();
    let n = name("stream");
    with_refs(&[&format!("@{n}")], |refs| {
        assert_eq!(
            handle_listen(&Ctx::new(c"listen".into(), refs, &[], &state)).unwrap(),
            0
        );
    });
    let mut buf = [0u8; sys::shellfd::TAG_MAX];
    let pid = sys::Pid::from_raw(std::process::id() as i32);
    let (fd, tag) = sys::shellfd::recv_fd(&receiver, &mut buf, pid).unwrap();
    fd.verify().unwrap();
    assert_eq!(tag.to_bytes(), b"listen");
    // A connected client makes the listening socket readable (POLLIN).
    let client = connect_abstract(&n);
    let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
    let nready = sys::poll::poll(&mut pfd, 2000).unwrap();
    assert_eq!(nready, 1);
    let revents = pfd.first().unwrap().revents;
    assert_ne!(revents & sys::poll::POLLIN, 0);
    drop(client);
    drop(fd);
}

#[test]
fn listen_handler_inet_sends_listening_socket() {
    let (state, receiver) = capture_state();
    let port: u16 = (20000 + std::process::id() % 10000) as u16;
    with_refs(
        &["--bind", "127.0.0.1", "--port", &format!("{port}")],
        |refs| {
            assert_eq!(
                handle_listen(&Ctx::new(c"listen".into(), refs, &[], &state)).unwrap(),
                0
            );
        },
    );
    let mut buf = [0u8; sys::shellfd::TAG_MAX];
    let pid = sys::Pid::from_raw(std::process::id() as i32);
    let (fd, tag) = sys::shellfd::recv_fd(&receiver, &mut buf, pid).unwrap();
    fd.verify().unwrap();
    assert_eq!(tag.to_bytes(), b"listen");
    let client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
    let nready = sys::poll::poll(&mut pfd, 2000).unwrap();
    assert_eq!(nready, 1);
    let revents = pfd.first().unwrap().revents;
    assert_ne!(revents & sys::poll::POLLIN, 0);
    drop(client);
    drop(fd);
}

#[test]
fn listen_handler_dgram_socket_is_syscall_error() {
    // `listen` on a dgram socket is EOPNOTSUPP (verified on this kernel).
    let (state, _receiver) = capture_state();
    let n = name("dgram");
    with_refs(&["--type", "dgram", &format!("@{n}")], |refs| {
        let e = handle_listen(&Ctx::new(c"listen".into(), refs, &[], &state)).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Syscall));
    });
}

#[test]
fn listen_handler_default_backlog_accepts_one_client() {
    let (state, receiver) = capture_state();
    let n = name("cap");
    with_refs(&[&format!("@{n}")], |refs| {
        assert_eq!(
            handle_listen(&Ctx::new(c"listen".into(), refs, &[], &state)).unwrap(),
            0
        );
    });
    let mut buf = [0u8; sys::shellfd::TAG_MAX];
    let pid = sys::Pid::from_raw(std::process::id() as i32);
    let (fd, _tag) = sys::shellfd::recv_fd(&receiver, &mut buf, pid).unwrap();
    fd.verify().unwrap();
    // Backlog 1: the first connection queues, a second must block; verify
    // only that the first lands.
    let client = connect_abstract(&n);
    let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
    let nready = sys::poll::poll(&mut pfd, 2000).unwrap();
    assert_eq!(nready, 1);
    drop(client);
    drop(fd);
}

#[test]
fn listen_handler_missing_address_is_missing_argument() {
    with_refs(&[], |refs| {
        let e =
            handle_listen(&Ctx::new(c"listen".into(), refs, &[], &ShellState::new())).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::MissingArgument("address")
        ));
    });
}

#[test]
fn listen_handler_bad_backlog_is_invalid_argument() {
    with_refs(&["--backlog", "2147483648", "x"], |refs| {
        let e =
            handle_listen(&Ctx::new(c"listen".into(), refs, &[], &ShellState::new())).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::InvalidArgument("backlog")
        ));
    });
}
