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

/// A test file inside a guarded per-test scratch dir: the returned pair is
/// the file path plus the dir's drop guard — bind both (`let (path, _scratch)
/// = …`). The old pid-named file in the temp-dir root leaked on the panic
/// path; the guard removes the whole dir when the test ends.
fn test_path() -> (PathBuf, Scratch) {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("fdshell-openat2-test-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.join("file"), Scratch(dir))
}

#[test]
fn open_with_create_writes() {
    let (path, _scratch) = test_path();
    let cpath = std::ffi::CString::new(path.to_str().unwrap()).unwrap();

    let fd = sys::openat2::open(&cpath, sys::fcntl::O_WRONLY | sys::fcntl::O_CREAT).unwrap();
    drop(fd);
    assert!(path.exists(), "file should have been created");
}
