#![allow(clippy::unwrap_used)]

use super::handle_fchmod;
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
    let result = handle_fchmod(c"fchmod".into(), &refs, &[], &ShellState::new());
    assert_eq!(result.unwrap(), 0);
    drop(local);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn fchmod_no_args_is_error() {
    let result = handle_fchmod(c"fchmod".into(), &[], &[], &ShellState::new());
    assert!(result.is_err());
}
