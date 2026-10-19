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
    let dir = std::env::temp_dir().join(format!("fdshell-renameat2-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

fn open_dir(path: &std::path::Path) -> sys::LocalFd {
    let cdir = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    // SAFETY: `cdir` is a valid NUL-terminated path; O_RDONLY|O_DIRECTORY|O_CLOEXEC
    // yields a valid dirfd with CLOEXEC set, satisfying the LocalFd invariant.
    let raw = unsafe {
        libc::open(
            cdir.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    assert!(raw >= 0);
    // SAFETY: `raw` is a valid open dirfd with CLOEXEC set.
    unsafe { sys::LocalFd::from_raw(raw) }
}

#[test]
fn renameat2_renames_file() {
    let dir = scratch();
    std::fs::write(dir.join("a"), b"x").unwrap();
    let dirfd = open_dir(&dir);
    sys::fileat::renameat2(dirfd.at(), c"a", dirfd.at(), c"b", 0).unwrap();
    // A stub returning Ok(()) without acting would leave "a" and drop "b".
    assert!(!dir.join("a").exists());
    assert!(dir.join("b").exists());
}

#[test]
fn renameat2_missing_source_errors() {
    let dir = scratch();
    let dirfd = open_dir(&dir);
    assert!(sys::fileat::renameat2(dirfd.at(), c"nope", dirfd.at(), c"b", 0).is_err());
}
