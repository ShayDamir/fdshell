#![allow(clippy::unwrap_used)]

use std::sync::atomic::AtomicU64;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn test_path() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("fdshell-openat2-test-{}-{}", std::process::id(), c))
}

#[test]
fn open_with_create_writes() {
    let path = test_path();
    let cpath = std::ffi::CString::new(path.to_str().unwrap()).unwrap();

    let fd = sys::openat2::open(&cpath, sys::fcntl::O_WRONLY | sys::fcntl::O_CREAT).unwrap();
    drop(fd);
    assert!(path.exists(), "file should have been created");
    let _ = std::fs::remove_file(&path);
}
