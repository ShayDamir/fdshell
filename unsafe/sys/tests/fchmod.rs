#![allow(clippy::unwrap_used)]

use std::ops::Deref;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use sys::ImportedFd;

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
    let dir = std::env::temp_dir().join(format!("fdshell-fchmod-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

#[test]
fn fchmod_changes_mode() {
    let dir = scratch();
    let file = dir.join("f");
    std::fs::write(&file, b"x").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
    let cpath = std::ffi::CString::new(file.to_str().unwrap()).unwrap();

    // SAFETY: `cpath` is a valid NUL-terminated path; O_RDWR without O_CLOEXEC
    // yields an fd with CLOEXEC clear, satisfying the ImportedFd invariant.
    let raw = unsafe { libc::open(cpath.as_ptr(), libc::O_RDWR) };
    assert!(raw >= 0);
    // SAFETY: `raw` is a valid open fd with CLOEXEC clear (opened above).
    let fd = unsafe { ImportedFd::from_raw(raw) };
    fd.fchmod(0o640).unwrap();
    // SAFETY: `raw` is the valid open fd created above.
    unsafe { libc::close(raw) };

    // A stub returning Ok(()) without acting would leave the mode at 0o700.
    let st = sys::stat::stat(&cpath).unwrap();
    assert_eq!(st.mode & 0o777, 0o640);
}

#[test]
fn fchmod_bad_fd_errors() {
    // SAFETY: -1 is never a valid fd; fchmod returns EBADF.
    let fd = unsafe { ImportedFd::from_raw(-1) };
    assert!(fd.fchmod(0o640).is_err());
}

#[test]
fn fstat_on_imported_fd_reports_the_real_mode() {
    let dir = scratch();
    let file = dir.join("f");
    std::fs::write(&file, b"x").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o640)).unwrap();
    let cpath = std::ffi::CString::new(file.to_str().unwrap()).unwrap();

    // SAFETY: `cpath` is a valid NUL-terminated path; O_RDWR without O_CLOEXEC
    // yields an fd with CLOEXEC clear, satisfying the ImportedFd invariant.
    let raw = unsafe { libc::open(cpath.as_ptr(), libc::O_RDWR) };
    assert!(raw >= 0);
    // SAFETY: `raw` is a valid open fd with CLOEXEC clear (opened above).
    let fd = unsafe { ImportedFd::from_raw(raw) };

    // The fd-side read of the same inode. `ImportedFd::fstat` had no caller
    // inside the `sys` package, so this is the only pin of it here; the mode is
    // the file's own, so an all-zero answer fails.
    assert_eq!(fd.fstat().unwrap().mode & 0o777, 0o640);
    // SAFETY: `raw` is the valid open fd created above.
    unsafe { libc::close(raw) };
}
