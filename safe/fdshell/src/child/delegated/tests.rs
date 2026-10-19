#![allow(clippy::unwrap_used)]

use super::{
    handle_eventfd, handle_fchmod, handle_symlinkat, handle_timerfd, handle_unlinkat,
    handle_utimensat,
};
use crate::child::Ctx;
use crate::state::ShellState;
use alloc::format;
use alloc::string::ToString;
use core::ffi::CStr;
use std::ffi::CString;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). The pid-named temp files live inside this dir now, so the
/// `remove_file` teardowns are the guard's job. Removal is `.ok()`-tolerant:
/// teardown never panics, and it runs on a failing test's unwind, which is
/// exactly the test that used to leak.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

impl Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A pid + counter scratch dir named after `tag` (the old
/// `fdshell-<tag>-<pid>-<counter>` files live inside it now), removed when
/// the test's guard drops.
fn scratch(tag: &str) -> Scratch {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-{tag}-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// A regular file inside a guarded per-test dir: the fd, its path, and the
/// dir's drop guard — bind all three.
fn temp_file() -> (sys::LocalFd, PathBuf, Scratch) {
    let dir = scratch("fchmod-test");
    let path = dir.join("f");
    let path_c = CString::new(path.to_str().unwrap()).unwrap();
    let fd = sys::openat2::open(
        path_c.as_c_str(),
        sys::fcntl::O_CREAT | sys::fcntl::O_WRONLY,
    )
    .unwrap();
    (fd, path, dir)
}

#[test]
fn fchmod_success_returns_zero() {
    let (local, _path, _scratch) = temp_file();
    let exported = local.export().unwrap();
    let fd_c = CString::new(exported.as_raw().to_string()).unwrap();
    let refs: [&CStr; 2] = [c"644", fd_c.as_c_str()];
    let result = handle_fchmod(&Ctx::new(c"fchmod".into(), &refs, &[], &ShellState::new()));
    assert_eq!(result.unwrap(), 0);
    drop(local);
}

#[test]
fn fchmod_no_args_is_error() {
    let result = handle_fchmod(&Ctx::new(c"fchmod".into(), &[], &[], &ShellState::new()));
    assert!(result.is_err());
}

#[test]
fn unlinkat_success_returns_zero() {
    let (local, path, _scratch) = temp_file();
    let path_c = CString::new(path.to_str().unwrap()).unwrap();
    let refs: [&CStr; 1] = [path_c.as_c_str()];
    let result = handle_unlinkat(&Ctx::new(
        c"unlinkat".into(),
        &refs,
        &[],
        &ShellState::new(),
    ));
    assert_eq!(result.unwrap(), 0);
    drop(local);
    assert!(!path.exists());
}

#[test]
fn unlinkat_no_args_is_error() {
    let result = handle_unlinkat(&Ctx::new(c"unlinkat".into(), &[], &[], &ShellState::new()));
    assert!(result.is_err());
}

#[test]
fn symlinkat_success_returns_zero() {
    let _scratch = scratch("symlinkat-handler");
    let link = _scratch.join("link");
    let link_c = CString::new(link.to_str().unwrap()).unwrap();
    let refs: [&CStr; 2] = [c"target", link_c.as_c_str()];
    let result = handle_symlinkat(&Ctx::new(
        c"symlinkat".into(),
        &refs,
        &[],
        &ShellState::new(),
    ));
    assert_eq!(result.unwrap(), 0);
    assert!(
        std::fs::symlink_metadata(&link).is_ok(),
        "link should exist"
    );
}

#[test]
fn symlinkat_no_args_is_error() {
    let result = handle_symlinkat(&Ctx::new(c"symlinkat".into(), &[], &[], &ShellState::new()));
    assert!(result.is_err());
}

#[test]
fn utimensat_success_returns_zero() {
    let (local, path, _scratch) = temp_file();
    let path_c = CString::new(path.to_str().unwrap()).unwrap();
    let refs: [&CStr; 3] = [c"--mtime", c"1234567890", path_c.as_c_str()];
    let result = handle_utimensat(&Ctx::new(
        c"utimensat".into(),
        &refs,
        &[],
        &ShellState::new(),
    ));
    assert_eq!(result.unwrap(), 0);
    drop(local);
}

#[test]
fn utimensat_no_args_is_error() {
    let result = handle_utimensat(&Ctx::new(c"utimensat".into(), &[], &[], &ShellState::new()));
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
    let (fd, tag) = receiver.recv_fd(&mut buf, pid).unwrap();
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
    let (fd, tag) = receiver.recv_fd(&mut buf, pid).unwrap();
    fd.verify().unwrap();
    assert_eq!(tag.to_bytes(), b"eventfd");

    // The non-zero initial counter makes the fd readable.
    let mut pfd = [sys::poll::PollFd::new(fd.as_raw(), sys::poll::POLLIN)];
    let n = sys::poll::poll(&mut pfd, 2000).unwrap();
    assert_eq!(n, 1);
    let revents = pfd.first().unwrap().revents;
    assert_ne!(revents & sys::poll::POLLIN, 0);
}
