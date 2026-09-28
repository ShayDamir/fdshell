#![allow(clippy::unwrap_used)]

use alloc::ffi::CString;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use builtins::error::BuiltinError;
use sys::{Origin, ShortCStr, Trace};

use crate::child::Ctx;
use crate::state::{FdVar, ShellState};

use super::handle_accept;

fn name(tag: &str) -> String {
    format!("fdshell-accept-{tag}-{}", std::process::id())
}

fn with_refs<R, F>(args: &[&str], f: F) -> R
where
    F: FnOnce(&[&core::ffi::CStr], &[ShortCStr]) -> R,
{
    let cs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let refs: Vec<&core::ffi::CStr> = cs.iter().map(|s| s.as_c_str()).collect();
    let origs: Vec<ShortCStr> = args
        .iter()
        .map(|a| ShortCStr::from_vec(a.as_bytes().to_vec()).unwrap())
        .collect();
    f(&refs, &origs)
}

fn connect_abstract(name: &str) -> std::os::unix::net::UnixStream {
    use std::os::linux::net::SocketAddrExt;
    let addr = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()).unwrap();
    std::os::unix::net::UnixStream::connect_addr(&addr).unwrap()
}

/// A listening abstract socket registered in `state` as `%l`, plus a
/// capture socketpair (parent side kept by the test).
fn listening_state() -> (ShellState, sys::LocalFd, String) {
    let n = name("srv");
    let n_c = CString::new(n.clone()).unwrap();
    let fd = sys::net::socket(sys::net::AF_UNIX, sys::net::SOCK_STREAM).unwrap();
    sys::net::bind_uds_abstract(&fd, &n_c).unwrap();
    sys::net::listen(&fd, 4).unwrap();
    let (shell_sock, receiver) = sys::net::socketpair().unwrap();
    shell_sock.verify().unwrap();
    receiver.verify().unwrap();
    sys::shellfd::set_capture_active(true);
    let mut state = ShellState::new();
    state.set_shell_sock(shell_sock);
    state.fds.insert(
        c"l".into(),
        FdVar {
            fd,
            trace: Trace::boundary(Origin::Shell),
        },
    );
    (state, receiver, n)
}

#[test]
fn accept_handler_sends_accepted_connection() {
    let (state, receiver, n) = listening_state();
    let client = connect_abstract(&n);
    with_refs(&["%l"], |refs, origs| {
        assert_eq!(
            handle_accept(&Ctx::new(c"accept".into(), refs, origs, &state)).unwrap(),
            0
        );
    });
    let mut buf = [0u8; sys::shellfd::TAG_MAX];
    let pid = sys::Pid::from_raw(std::process::id() as i32);
    let (fd, tag) = sys::shellfd::recv_fd(&receiver, &mut buf, pid).unwrap();
    fd.verify().unwrap();
    assert_eq!(tag.to_bytes(), b"accept");
    drop(client);
    drop(fd);
}

#[test]
fn accept_handler_unknown_var_is_fdvar_not_found() {
    let (state, _receiver, _n) = listening_state();
    with_refs(&["%nope"], |refs, origs| {
        let e = handle_accept(&Ctx::new(c"accept".into(), refs, origs, &state)).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::FdVarNotFound));
    });
}

#[test]
fn accept_handler_missing_arg_is_missing_argument() {
    let (state, _receiver, _n) = listening_state();
    with_refs(&[], |refs, origs| {
        let e = handle_accept(&Ctx::new(c"accept".into(), refs, origs, &state)).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::MissingArgument("fd var")
        ));
    });
}

#[test]
fn accept_handler_bare_number_is_invalid_argument() {
    let (state, _receiver, _n) = listening_state();
    with_refs(&["3"], |refs, origs| {
        let e = handle_accept(&Ctx::new(c"accept".into(), refs, origs, &state)).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::InvalidArgument("fd var")
        ));
    });
}

#[test]
fn accept_handler_help_bails_help() {
    let (state, _receiver, _n) = listening_state();
    with_refs(&["--help"], |refs, origs| {
        let e = handle_accept(&Ctx::new(c"accept".into(), refs, origs, &state)).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Help));
    });
}

#[test]
fn accept_handler_extra_arg_is_invalid_argument() {
    let (state, _receiver, _n) = listening_state();
    with_refs(&["%l", "junk"], |refs, origs| {
        let e = handle_accept(&Ctx::new(c"accept".into(), refs, origs, &state)).unwrap_err();
        assert!(matches!(
            e.current_context(),
            BuiltinError::InvalidArgument("arg")
        ));
    });
}

#[test]
fn accept_handler_on_non_listening_socket_is_syscall_error() {
    // A bound (not listening) stream socket rejects accept with EINVAL.
    let n = name("nope");
    let n_c = CString::new(n).unwrap();
    let fd = sys::net::socket(sys::net::AF_UNIX, sys::net::SOCK_STREAM).unwrap();
    sys::net::bind_uds_abstract(&fd, &n_c).unwrap();
    let (shell_sock, receiver) = sys::net::socketpair().unwrap();
    shell_sock.verify().unwrap();
    receiver.verify().unwrap();
    sys::shellfd::set_capture_active(true);
    let mut state = ShellState::new();
    state.set_shell_sock(shell_sock);
    state.fds.insert(
        c"l".into(),
        FdVar {
            fd,
            trace: Trace::boundary(Origin::Shell),
        },
    );
    with_refs(&["%l"], |refs, origs| {
        let e = handle_accept(&Ctx::new(c"accept".into(), refs, origs, &state)).unwrap_err();
        assert!(matches!(e.current_context(), BuiltinError::Syscall));
    });
}
