#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use sys::fcntl::{LOCK_EX, LOCK_NB, LOCK_SH, LOCK_UN};
use sys::openat2::open;

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

/// A scratch dir with a regular file `lock`; each test gets its own dir.
/// The returned pair is the lock path plus the dir's drop guard — bind both
/// (`let (path, _scratch) = …`), the guard names the test's scratch.
fn lock_file() -> (PathBuf, Scratch) {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-flock-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lock"), b"").unwrap();
    (dir.join("lock"), Scratch(dir))
}

fn open_lock(path: &std::path::Path) -> sys::LocalFd {
    let cpath = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    open(cpath.as_c_str(), libc::O_RDWR | libc::O_CLOEXEC).unwrap()
}

#[test]
fn exclusive_lock_succeeds_and_releases() {
    let (path, _scratch) = lock_file();
    let fd = open_lock(&path);
    fd.flock(LOCK_EX).unwrap();
    fd.flock(LOCK_UN).unwrap();
}

#[test]
fn second_exclusive_nowait_is_ewouldblock() {
    let (path, _scratch) = lock_file();
    let fd1 = open_lock(&path);
    fd1.flock(LOCK_EX).unwrap();
    let fd2 = open_lock(&path);
    let err = fd2.flock(LOCK_EX + LOCK_NB).unwrap_err();
    assert_eq!(err.errno(), libc::EWOULDBLOCK);
}

#[test]
fn shared_locks_do_not_conflict() {
    let (path, _scratch) = lock_file();
    let fd1 = open_lock(&path);
    fd1.flock(LOCK_SH).unwrap();
    let fd2 = open_lock(&path);
    fd2.flock(LOCK_SH + LOCK_NB).unwrap();
}

#[test]
fn shared_conflicts_with_exclusive_nowait() {
    let (path, _scratch) = lock_file();
    let fd1 = open_lock(&path);
    fd1.flock(LOCK_SH).unwrap();
    let fd2 = open_lock(&path);
    let err = fd2.flock(LOCK_EX + LOCK_NB).unwrap_err();
    assert_eq!(err.errno(), libc::EWOULDBLOCK);
}

#[test]
fn unlock_allows_exclusive_nowait() {
    let (path, _scratch) = lock_file();
    let fd1 = open_lock(&path);
    fd1.flock(LOCK_EX).unwrap();
    fd1.flock(LOCK_UN).unwrap();
    let fd2 = open_lock(&path);
    fd2.flock(LOCK_EX + LOCK_NB).unwrap();
}

#[test]
fn dup_of_locked_fd_does_not_conflict() {
    // A dup shares the open file description, so flock sees one lock holder.
    let (path, _scratch) = lock_file();
    let fd1 = open_lock(&path);
    fd1.flock(LOCK_EX).unwrap();
    let fd2 = sys::dup::dup_cloexec(fd1.as_raw()).unwrap();
    fd2.flock(LOCK_EX + LOCK_NB).unwrap();
}
