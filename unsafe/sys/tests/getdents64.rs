#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use std::ffi::CString;
use std::ops::Deref;
use std::path::PathBuf;

use sys::getdents64::{Iter, getdents};
use sys::openat2::open;

/// `d_type` value for a directory, derived from `S_IFDIR` via the kernel's
/// `IFTODT` rule (`(mode & 0170000) >> 12`), so it stays correct regardless of
/// the platform's `DT_DIR` encoding.
const DT_DIR: u32 = (sys::stat::S_IFDIR & 0o170000) >> 12;

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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

/// A scratch directory holding regular files `a`, `b`, `c` and a subdir `sub`;
/// each test gets its own dir, removed when the test's guard drops.
fn scratch() -> Scratch {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-gd64-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["a", "b", "c"] {
        std::fs::write(dir.join(name), name.as_bytes()).unwrap();
    }
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    Scratch(dir)
}

fn cstr(p: &std::path::Path) -> CString {
    CString::new(p.to_str().unwrap()).unwrap()
}

/// Collect every entry name under an open directory by driving
/// `LocalFd::dir_batch` until it reports the directory exhausted. `dir_fd`
/// is the owning `LocalFd`, kept alive for the duration of the call.
fn list_all(dir_fd: &sys::LocalFd) -> Vec<Vec<u8>> {
    let mut names = Vec::new();
    while let Some(batch) = dir_fd.dir_batch().unwrap() {
        for entry in batch.iter() {
            names.push(entry.name.to_vec());
        }
    }
    names
}

#[test]
fn lists_all_entries_including_dot_dirs() {
    let dir = scratch();
    let dir_fd = open(cstr(&dir).as_c_str(), libc::O_RDONLY | libc::O_CLOEXEC).unwrap();
    let names: Vec<String> = list_all(&dir_fd)
        .into_iter()
        .map(|b| core::str::from_utf8(&b).unwrap().to_string())
        .collect();
    assert!(names.contains(&".".to_string()));
    assert!(names.contains(&"..".to_string()));
    assert!(names.contains(&"a".to_string()));
    assert!(names.contains(&"b".to_string()));
    assert!(names.contains(&"c".to_string()));
    assert!(names.contains(&"sub".to_string()));
}

#[test]
fn reports_type_and_reclen_for_a_record() {
    let dir = scratch();
    let dir_fd = open(cstr(&dir).as_c_str(), libc::O_RDONLY | libc::O_CLOEXEC).unwrap();
    let mut buf = [0u8; 4096];
    let n = getdents(&dir_fd, &mut buf).unwrap();
    assert!(n > 0);
    let mut off = 0usize;
    let mut found_dot = false;
    while off + 18 <= n {
        let reclen = u16::from_le_bytes(buf[off + 16..off + 18].try_into().unwrap()) as usize;
        let entry = Iter::new(buf.get(off..off + reclen).unwrap(), reclen)
            .next()
            .unwrap();
        if entry.name == b"." {
            // `.` is a directory: header (24 bytes) plus a one-byte name and its NUL.
            assert_eq!(u32::from(entry.d_type), DT_DIR);
            assert!(reclen >= 24);
            found_dot = true;
            break;
        }
        off += reclen;
    }
    assert!(found_dot, "no `.` record in the first batch");
}

#[test]
fn read_after_eof_returns_zero() {
    let dir = scratch();
    let dir_fd = open(cstr(&dir).as_c_str(), libc::O_RDONLY | libc::O_CLOEXEC).unwrap();
    let mut big = [0u8; 8192];
    while getdents(&dir_fd, &mut big).unwrap() != 0 {}
    // The file offset is now at EOF; a further read yields zero.
    let mut buf = [0u8; 256];
    assert_eq!(getdents(&dir_fd, &mut buf).unwrap(), 0);
}

#[test]
fn non_directory_is_enotdir() {
    let dir = scratch();
    let file_fd = open(
        cstr(&dir.join("a")).as_c_str(),
        libc::O_RDONLY | libc::O_CLOEXEC,
    )
    .unwrap();
    let mut buf = [0u8; 4096];
    let err = getdents(&file_fd, &mut buf).unwrap_err();
    assert_eq!(err.errno(), libc::ENOTDIR);
}

#[test]
fn empty_directory_lists_only_dot_dirs() {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-gd64-empty-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    let _guard = Scratch(dir.clone());
    let dir_fd = open(cstr(&dir).as_c_str(), libc::O_RDONLY | libc::O_CLOEXEC).unwrap();
    let names: Vec<String> = list_all(&dir_fd)
        .into_iter()
        .map(|b| core::str::from_utf8(&b).unwrap().to_string())
        .collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&".".to_string()));
    assert!(names.contains(&"..".to_string()));
}

/// Scratch dir with 256 files `file-000`..`file-255` (~8 KiB of records), so
/// a 4 KiB buffer needs more than one `getdents64` pass.
fn big_scratch() -> Scratch {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-gd64-big-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..256u32 {
        std::fs::write(dir.join(format!("file-{:03}", i)), b"").unwrap();
    }
    Scratch(dir)
}

#[test]
fn dir_batch_walks_more_than_one_buffer() {
    let dir = big_scratch();
    let dir_fd = open(cstr(&dir).as_c_str(), libc::O_RDONLY | libc::O_CLOEXEC).unwrap();
    let mut batches = 0;
    let mut names = Vec::new();
    while let Some(batch) = dir_fd.dir_batch().unwrap() {
        batches += 1;
        for entry in batch.iter() {
            names.push(entry.name.to_vec());
        }
    }
    assert!(batches >= 2, "256 entries need more than one 4 KiB buffer");
    for i in 0..256u32 {
        let name = format!("file-{:03}", i).into_bytes();
        assert!(
            names.contains(&name),
            "missing {}",
            String::from_utf8_lossy(&name)
        );
    }
}

#[test]
fn dir_batch_returns_none_at_end() {
    let dir = scratch();
    let dir_fd = open(cstr(&dir).as_c_str(), libc::O_RDONLY | libc::O_CLOEXEC).unwrap();
    while dir_fd.dir_batch().unwrap().is_some() {}
    assert!(dir_fd.dir_batch().unwrap().is_none());
}

#[test]
fn dir_batch_on_non_directory_is_enotdir() {
    let dir = scratch();
    let file_fd = open(
        cstr(&dir.join("a")).as_c_str(),
        libc::O_RDONLY | libc::O_CLOEXEC,
    )
    .unwrap();
    let err = file_fd.dir_batch().unwrap_err();
    assert_eq!(err.errno(), libc::ENOTDIR);
}

#[test]
fn dir_batch_iter_yields_dot_with_dir_type() {
    let dir = scratch();
    let dir_fd = open(cstr(&dir).as_c_str(), libc::O_RDONLY | libc::O_CLOEXEC).unwrap();
    let batch = dir_fd.dir_batch().unwrap().unwrap();
    let mut found_dot = false;
    for entry in batch.iter() {
        if entry.name == b"." {
            assert_eq!(u32::from(entry.d_type), DT_DIR);
            found_dot = true;
            break;
        }
    }
    assert!(found_dot);
}
