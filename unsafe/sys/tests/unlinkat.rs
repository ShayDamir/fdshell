#![allow(clippy::unwrap_used)]

use std::sync::atomic::AtomicU64;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn test_dir() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("fdshell-unlinkat-test-{}-{}", std::process::id(), c))
}

#[test]
fn unlinkat_file() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("testfile");
    std::fs::write(&file, b"hello").unwrap();
    assert!(file.exists());

    let path = std::ffi::CString::new(file.to_str().unwrap()).unwrap();
    sys::unlinkat::unlinkat(sys::AtFd::cwd(), &path, 0).unwrap();
    assert!(!file.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unlinkat_dir() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let sub = dir.join("subdir");
    std::fs::create_dir_all(&sub).unwrap();
    assert!(sub.exists());

    let path = std::ffi::CString::new(sub.to_str().unwrap()).unwrap();
    sys::unlinkat::unlinkat(sys::AtFd::cwd(), &path, sys::unlinkat::AT_REMOVEDIR).unwrap();
    assert!(!sub.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unlinkat_enoent() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let missing = dir.join("does-not-exist");
    let path = std::ffi::CString::new(missing.to_str().unwrap()).unwrap();
    let err = sys::unlinkat::unlinkat(sys::AtFd::cwd(), &path, 0).unwrap_err();
    assert_eq!(err, libc::ENOENT);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unlinkat_notdir() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("afile");
    std::fs::write(&file, b"x").unwrap();
    // AT_REMOVEDIR on a regular file should fail with ENOTDIR
    let path = std::ffi::CString::new(file.to_str().unwrap()).unwrap();
    let err =
        sys::unlinkat::unlinkat(sys::AtFd::cwd(), &path, sys::unlinkat::AT_REMOVEDIR).unwrap_err();
    assert_eq!(err, libc::ENOTDIR);
    let _ = std::fs::remove_dir_all(&dir);
}
