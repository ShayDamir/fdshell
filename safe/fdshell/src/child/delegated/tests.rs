#![allow(clippy::unwrap_used)]

use super::{handle_eventfd, handle_fchmod, handle_timerfd};
use crate::child::Ctx;
use crate::state::ShellState;
use alloc::format;
use alloc::string::ToString;
use core::ffi::CStr;
use std::ffi::CString;
use std::sync::atomic::{AtomicU64, Ordering};

fn temp_file() -> (sys::LocalFd, std::path::PathBuf) {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("fdshell-fchmod-test-{}-{}", std::process::id(), c));
    let path_c = CString::new(path.to_str().unwrap()).unwrap();
    let fd = sys::openat2::open(
        path_c.as_c_str(),
        sys::fcntl::O_CREAT | sys::fcntl::O_WRONLY,
    )
    .unwrap();
    (fd, path)
}

#[test]
fn fchmod_success_returns_zero() {
    let (local, path) = temp_file();
    let exported = local.export().unwrap();
    let fd_c = CString::new(exported.as_raw().to_string()).unwrap();
    let refs: [&CStr; 2] = [c"644", fd_c.as_c_str()];
    let result = handle_fchmod(&Ctx::new(c"fchmod".into(), &refs, &[], &ShellState::new()));
    assert_eq!(result.unwrap(), 0);
    drop(local);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn fchmod_no_args_is_error() {
    let result = handle_fchmod(&Ctx::new(c"fchmod".into(), &[], &[], &ShellState::new()));
    assert!(result.is_err());
}

#[test]
fn timerfd_no_args_is_error() {
    let result = handle_timerfd(&Ctx::new(c"timerfd".into(), &[], &[], &ShellState::new()));
    assert!(result.is_err());
}

#[test]
fn eventfd_no_args_is_error() {
    let result = handle_eventfd(&Ctx::new(c"eventfd".into(), &[], &[], &ShellState::new()));
    assert!(result.is_err());
}

#[test]
fn timerfd_success_sends_fd() {
    let (shell_sock, receiver) = sys::net::socketpair().unwrap();
    shell_sock.verify().unwrap();
    receiver.verify().unwrap();
    sys::shellfd::set_capture_active(true);

    let mut state = ShellState::new();
    state.set_shell_sock(shell_sock);

    // A one-shot timer armed ~10ms out.
    let refs: [&CStr; 2] = [c"0", c"10000000"];
    let result = handle_timerfd(&Ctx::new(c"timerfd".into(), &refs, &[], &state));
    assert_eq!(result.unwrap(), 0);

    let mut buf = [0u8; sys::shellfd::TAG_MAX];
    let pid = sys::Pid::from_raw(std::process::id() as i32);
    let (fd, tag) = sys::shellfd::recv_fd(&receiver, &mut buf, pid).unwrap();
    fd.verify().unwrap();
    assert_eq!(tag.to_bytes(), b"timerfd");

    // The one-shot timer fires after ~10ms, making the fd readable.
    let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
    let n = sys::poll::poll(&mut pfd, 2000).unwrap();
    assert_eq!(n, 1);
    let revents = pfd.first().unwrap().revents;
    assert_ne!(revents & sys::poll::POLLIN, 0);
}

#[test]
fn eventfd_success_sends_fd() {
    let (shell_sock, receiver) = sys::net::socketpair().unwrap();
    shell_sock.verify().unwrap();
    receiver.verify().unwrap();
    sys::shellfd::set_capture_active(true);

    let mut state = ShellState::new();
    state.set_shell_sock(shell_sock);

    let refs: [&CStr; 1] = [c"1"];
    let result = handle_eventfd(&Ctx::new(c"eventfd".into(), &refs, &[], &state));
    assert_eq!(result.unwrap(), 0);

    let mut buf = [0u8; sys::shellfd::TAG_MAX];
    let pid = sys::Pid::from_raw(std::process::id() as i32);
    let (fd, tag) = sys::shellfd::recv_fd(&receiver, &mut buf, pid).unwrap();
    fd.verify().unwrap();
    assert_eq!(tag.to_bytes(), b"eventfd");

    // The non-zero initial counter makes the fd readable.
    let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
    let n = sys::poll::poll(&mut pfd, 2000).unwrap();
    assert_eq!(n, 1);
    let revents = pfd.first().unwrap().revents;
    assert_ne!(revents & sys::poll::POLLIN, 0);
}
