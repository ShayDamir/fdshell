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
    let dir = std::env::temp_dir().join(format!("fdshell-mkfifoat-{}-{}", std::process::id(), c));
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
fn mkfifoat_creates_fifo() {
    let dir = scratch();
    let dirfd = open_dir(&dir);
    sys::fileat::mkfifoat(dirfd.at(), c"pipe", 0o600).unwrap();
    // A stub returning Ok(()) without acting would leave the fifo absent.
    let st = sys::stat::stat(&std::ffi::CString::new(dir.join("pipe").to_str().unwrap()).unwrap())
        .unwrap();
    assert_eq!(st.mode & sys::stat::S_IFMT, sys::stat::S_IFIFO);
}

#[test]
fn mkfifoat_existing_path_is_eexist() {
    let dir = scratch();
    std::fs::File::create(dir.join("pipe")).unwrap();
    let dirfd = open_dir(&dir);
    let err = sys::fileat::mkfifoat(dirfd.at(), c"pipe", 0o600).unwrap_err();
    assert_eq!(err.errno(), libc::EEXIST);
}

#[test]
fn mkfifoat_missing_parent_is_enoent() {
    let dir = scratch();
    let dirfd = open_dir(&dir);
    let err = sys::fileat::mkfifoat(dirfd.at(), c"no/such/pipe", 0o600).unwrap_err();
    assert_eq!(err.errno(), libc::ENOENT);
}

#[test]
fn mkfifoat_regular_file_dirfd_is_enotdir() {
    let dir = scratch();
    std::fs::write(dir.join("f"), b"x").unwrap();
    let cfile = std::ffi::CString::new(dir.join("f").to_str().unwrap()).unwrap();
    // SAFETY: `cfile` is a valid NUL-terminated path.
    let reg = unsafe { libc::open(cfile.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    assert!(reg >= 0);
    // SAFETY: `reg` is a valid open fd with CLOEXEC set.
    let regfd = unsafe { sys::LocalFd::from_raw(reg) };
    let err = sys::fileat::mkfifoat(regfd.at(), c"pipe", 0o600).unwrap_err();
    assert_eq!(err.errno(), libc::ENOTDIR);
}

#[test]
fn mkfifoat_invalid_dirfd_is_ebadf() {
    let fd = sys::memfd::memfd_create().unwrap();
    let raw = fd.as_raw();
    drop(fd);
    // SAFETY: `raw` is a just-closed fd number with no live owner, so
    // wrapping it in a non-owning AtFd is harmless. The number is invalid,
    // so `mkfifoat` sees EBADF.
    let ghost = unsafe { sys::AtFd::from_raw(raw) };
    let err = sys::fileat::mkfifoat(ghost, c"pipe", 0o600).unwrap_err();
    assert_eq!(err.errno(), libc::EBADF);
}
