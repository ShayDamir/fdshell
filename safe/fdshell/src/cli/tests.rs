#![allow(clippy::unwrap_used)]

use super::load_script;
use crate::AppError;

/// A memfd pre-filled with `data` and seeked back to the start, so
/// `load_script` reads from byte 0.
fn memfd_bytes(data: &[u8]) -> sys::LocalFd {
    let fd = sys::memfd::memfd_create().unwrap();
    sys::rw::write(&fd, data).unwrap();
    sys::rw::lseek(&fd, 0, sys::fcntl::SEEK_SET).unwrap();
    fd
}

#[test]
fn load_script_under_limit_returns_all() {
    // 8192 bytes forces two 4 KiB reads through the cap check.
    let data = [0xABu8; 8192];
    let fd = memfd_bytes(&data);
    let content = load_script(&fd, 65536).unwrap();
    assert_eq!(content.as_slice(), &data[..]);
}

#[test]
fn load_script_at_limit_returns_all() {
    let data = [0xABu8; 8192];
    let fd = memfd_bytes(&data);
    let content = load_script(&fd, 8192).unwrap();
    assert_eq!(content.as_slice(), &data[..]);
}

#[test]
fn load_script_over_limit_fails() {
    let data = [0xABu8; 8192];
    let fd = memfd_bytes(&data);
    let e = load_script(&fd, 4096).unwrap_err();
    assert!(matches!(e.current_context(), AppError::ScriptTooLarge));
}
