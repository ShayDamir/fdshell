#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use std::ffi::CString;
use sys::fcntl::{AT_EMPTY_PATH, AT_SYMLINK_NOFOLLOW};
use sys::openat2::open;
use sys::stat::{S_IFDIR, S_IFLNK, S_IFMT, S_IFREG};
use sys::statx::statx;

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A scratch dir with a regular file `f` (5 bytes) and a subdirectory `sub`;
/// each test gets its own dir.
fn scratch() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fdshell-statx-{}-{}", std::process::id(), c));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f"), b"12345").unwrap();
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub").join("g"), b"67890").unwrap();
    dir
}

fn cstr(p: &std::path::Path) -> CString {
    CString::new(p.to_str().unwrap()).unwrap()
}

#[test]
fn file_metadata() {
    let dir = scratch();
    let st = statx(sys::AtFd::cwd(), cstr(&dir.join("f")).as_c_str(), 0).unwrap();
    assert_eq!(st.mode & S_IFMT, S_IFREG);
    assert_eq!(st.size, 5);
    assert!(st.ino > 0);
    assert!(st.mtime_sec > 0);
}

#[test]
fn directory_metadata() {
    let dir = scratch();
    let st = statx(sys::AtFd::cwd(), cstr(&dir.join("sub")).as_c_str(), 0).unwrap();
    assert_eq!(st.mode & S_IFMT, S_IFDIR);
}

#[test]
fn relative_path_resolves_against_dirfd() {
    let dir = scratch();
    let sub = sys::openat2::open(
        cstr(&dir.join("sub")).as_c_str(),
        libc::O_RDONLY | libc::O_CLOEXEC,
    )
    .unwrap();
    let st = statx(sub.at(), c"g", 0).unwrap();
    assert_eq!(st.mode & S_IFMT, S_IFREG);
    assert_eq!(st.size, 5);
}

#[test]
fn symlink_followed_and_lstat() {
    let dir = scratch();
    std::os::unix::fs::symlink(dir.join("f"), dir.join("link")).unwrap();
    let link_c = cstr(&dir.join("link"));
    let link = link_c.as_c_str();
    let followed = statx(sys::AtFd::cwd(), link, 0).unwrap();
    assert_eq!(followed.mode & S_IFMT, S_IFREG);
    assert_eq!(followed.size, 5);
    let lstat = statx(sys::AtFd::cwd(), link, AT_SYMLINK_NOFOLLOW).unwrap();
    assert_eq!(lstat.mode & S_IFMT, S_IFLNK);
}

#[test]
fn empty_path_restats_the_open_handle() {
    let dir = scratch();
    let path = dir.join("f");
    let fd = open(cstr(&path).as_c_str(), libc::O_RDWR | libc::O_CLOEXEC).unwrap();
    std::fs::write(&path, b"1234567890").unwrap();
    let st = statx(fd.at(), c"", AT_EMPTY_PATH).unwrap();
    assert_eq!(st.mode & S_IFMT, S_IFREG);
    assert_eq!(st.size, 10);
}

#[test]
fn missing_path_is_enoent() {
    let dir = scratch();
    let err = statx(sys::AtFd::cwd(), cstr(&dir.join("nope")).as_c_str(), 0).unwrap_err();
    assert_eq!(err.errno(), libc::ENOENT);
}

#[test]
fn regular_file_dirfd_is_enotdir() {
    let dir = scratch();
    let fd = open(
        cstr(&dir.join("f")).as_c_str(),
        libc::O_RDONLY | libc::O_CLOEXEC,
    )
    .unwrap();
    let err = statx(fd.at(), c"g", 0).unwrap_err();
    assert_eq!(err.errno(), libc::ENOTDIR);
}
