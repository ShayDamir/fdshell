#![allow(clippy::unwrap_used)]

use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A per-test scratch dir (pid + counter), removed on drop. pid + counter is
/// unique among *live* processes only, so the scratch must die with its
/// process: pids are recycled, and a dir that outlives its process is a later
/// run's inherited dir with that run's files in it (LESSONS, task #255; sweep
/// task #342). Removal is `.ok()`-tolerant: teardown never panics, and it runs
/// on a failing test's unwind, which is exactly the test that used to leak.
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

fn scratch() -> Scratch {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fdshell-unlinkat-test-{}-{}",
        std::process::id(),
        c
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

#[test]
fn unlinkat_file() {
    let dir = scratch();
    let file = dir.join("testfile");
    std::fs::write(&file, b"hello").unwrap();
    assert!(file.exists());

    let path = std::ffi::CString::new(file.to_str().unwrap()).unwrap();
    sys::fileat::unlinkat(sys::AtFd::cwd(), &path, 0).unwrap();
    assert!(!file.exists());
}

#[test]
fn unlinkat_dir() {
    let dir = scratch();
    let sub = dir.join("subdir");
    std::fs::create_dir_all(&sub).unwrap();
    assert!(sub.exists());

    let path = std::ffi::CString::new(sub.to_str().unwrap()).unwrap();
    sys::fileat::unlinkat(sys::AtFd::cwd(), &path, sys::fileat::AT_REMOVEDIR).unwrap();
    assert!(!sub.exists());
}

#[test]
fn unlinkat_enoent() {
    let dir = scratch();
    let missing = dir.join("does-not-exist");
    let path = std::ffi::CString::new(missing.to_str().unwrap()).unwrap();
    let err = sys::fileat::unlinkat(sys::AtFd::cwd(), &path, 0).unwrap_err();
    assert_eq!(err, sys::SyscallError::ENOENT("unknown"));
}

#[test]
fn unlinkat_notdir() {
    let dir = scratch();
    let file = dir.join("afile");
    std::fs::write(&file, b"x").unwrap();
    // AT_REMOVEDIR on a regular file should fail with ENOTDIR
    let path = std::ffi::CString::new(file.to_str().unwrap()).unwrap();
    let err =
        sys::fileat::unlinkat(sys::AtFd::cwd(), &path, sys::fileat::AT_REMOVEDIR).unwrap_err();
    assert_eq!(
        err,
        sys::SyscallError::Other {
            errno: libc::ENOTDIR,
            syscall: "unknown"
        }
    );
}
