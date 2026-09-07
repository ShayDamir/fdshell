#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use std::ffi::CString;
use std::path::PathBuf;

use sys::getdents64::{Iter, getdents};
use sys::openat2::open;

/// `d_type` value for a directory, derived from `S_IFDIR` via the kernel's
/// `IFTODT` rule (`(mode & 0170000) >> 12`), so it stays correct regardless of
/// the platform's `DT_DIR` encoding.
const DT_DIR: u32 = (sys::stat::S_IFDIR & 0o170000) >> 12;

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A scratch directory holding regular files `a`, `b`, `c` and a subdir `sub`;
/// each test gets its own dir.
fn scratch() -> PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-gd64-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["a", "b", "c"] {
        std::fs::write(dir.join(name), name.as_bytes()).unwrap();
    }
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    dir
}

fn cstr(p: &std::path::Path) -> CString {
    CString::new(p.to_str().unwrap()).unwrap()
}

/// Collect every entry name under an open directory by repeatedly calling
/// `getdents` and iterating each returned buffer. `dir_fd` is the owning
/// `LocalFd`, kept alive for the duration of the call.
fn list_all(dir_fd: &sys::LocalFd) -> Vec<Vec<u8>> {
    let mut names = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = getdents(dir_fd.as_raw(), &mut buf).unwrap();
        if n == 0 {
            break;
        }
        for entry in Iter::new(&buf, n) {
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
    let n = getdents(dir_fd.as_raw(), &mut buf).unwrap();
    assert!(n > 0);
    let first = Iter::new(&buf, n).next().unwrap();
    // First record is always `.`: a directory named one byte.
    assert_eq!(u32::from(first.d_type), DT_DIR);
    assert_eq!(first.name, b".");
    // `d_reclen` covers the fixed header plus the name and its NUL terminator.
    let reclen = u16::from_le_bytes(buf[16..18].try_into().unwrap());
    assert!(reclen as usize >= 24);
}

#[test]
fn read_after_eof_returns_zero() {
    let dir = scratch();
    let dir_fd = open(cstr(&dir).as_c_str(), libc::O_RDONLY | libc::O_CLOEXEC).unwrap();
    let mut big = [0u8; 8192];
    while getdents(dir_fd.as_raw(), &mut big).unwrap() != 0 {}
    // The file offset is now at EOF; a further read yields zero.
    let mut buf = [0u8; 256];
    assert_eq!(getdents(dir_fd.as_raw(), &mut buf).unwrap(), 0);
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
    let err = getdents(file_fd.as_raw(), &mut buf).unwrap_err();
    assert_eq!(err.errno(), libc::ENOTDIR);
}

#[test]
fn empty_directory_lists_only_dot_dirs() {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-gd64-empty-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    let dir_fd = open(cstr(&dir).as_c_str(), libc::O_RDONLY | libc::O_CLOEXEC).unwrap();
    let names: Vec<String> = list_all(&dir_fd)
        .into_iter()
        .map(|b| core::str::from_utf8(&b).unwrap().to_string())
        .collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&".".to_string()));
    assert!(names.contains(&"..".to_string()));
}
