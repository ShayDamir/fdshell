#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use std::sync::atomic::AtomicU64;
use sys::fcntl::{LOCK_EX, LOCK_NB, LOCK_SH, LOCK_UN};
use sys::openat2::open;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A scratch dir with a regular file `lock`; each test gets its own dir.
fn lock_file() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-flock-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lock"), b"").unwrap();
    dir.join("lock")
}

fn open_lock(path: &std::path::Path) -> sys::LocalFd {
    let cpath = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    open(cpath.as_c_str(), libc::O_RDWR | libc::O_CLOEXEC).unwrap()
}

#[test]
fn exclusive_lock_succeeds_and_releases() {
    let path = lock_file();
    let fd = open_lock(&path);
    sys::flock::flock(&fd, LOCK_EX).unwrap();
    sys::flock::flock(&fd, LOCK_UN).unwrap();
}

#[test]
fn second_exclusive_nowait_is_ewouldblock() {
    let path = lock_file();
    let fd1 = open_lock(&path);
    sys::flock::flock(&fd1, LOCK_EX).unwrap();
    let fd2 = open_lock(&path);
    let err = sys::flock::flock(&fd2, LOCK_EX + LOCK_NB).unwrap_err();
    assert_eq!(err.errno(), libc::EWOULDBLOCK);
}

#[test]
fn shared_locks_do_not_conflict() {
    let path = lock_file();
    let fd1 = open_lock(&path);
    sys::flock::flock(&fd1, LOCK_SH).unwrap();
    let fd2 = open_lock(&path);
    sys::flock::flock(&fd2, LOCK_SH + LOCK_NB).unwrap();
}

#[test]
fn shared_conflicts_with_exclusive_nowait() {
    let path = lock_file();
    let fd1 = open_lock(&path);
    sys::flock::flock(&fd1, LOCK_SH).unwrap();
    let fd2 = open_lock(&path);
    let err = sys::flock::flock(&fd2, LOCK_EX + LOCK_NB).unwrap_err();
    assert_eq!(err.errno(), libc::EWOULDBLOCK);
}

#[test]
fn unlock_allows_exclusive_nowait() {
    let path = lock_file();
    let fd1 = open_lock(&path);
    sys::flock::flock(&fd1, LOCK_EX).unwrap();
    sys::flock::flock(&fd1, LOCK_UN).unwrap();
    let fd2 = open_lock(&path);
    sys::flock::flock(&fd2, LOCK_EX + LOCK_NB).unwrap();
}

#[test]
fn dup_of_locked_fd_does_not_conflict() {
    // A dup shares the open file description, so flock sees one lock holder.
    let path = lock_file();
    let fd1 = open_lock(&path);
    sys::flock::flock(&fd1, LOCK_EX).unwrap();
    let fd2 = sys::dup::dup_cloexec(fd1.as_raw()).unwrap();
    sys::flock::flock(&fd2, LOCK_EX + LOCK_NB).unwrap();
}
