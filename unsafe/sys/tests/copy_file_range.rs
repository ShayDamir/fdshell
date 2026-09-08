#![allow(clippy::unwrap_used)]

use sys::LocalFd;

/// A fresh temp dir for one test; removed on drop. Keeps the regular-file
/// tests independent of `memfd_create`, which is unavailable in this dev
/// container.
struct Dir(std::path::PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("fdshell-cfra-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn path(&self, name: &str) -> std::path::PathBuf {
        self.0.join(name)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Open `path` with `flags | O_CLOEXEC` and `mode`, returning a `LocalFd`.
///
/// `mode` is passed so `O_CREAT` files get readable permissions rather than
/// mode `0` (the varargs default when `libc::open` is called without it).
fn open(path: &std::path::Path, flags: i32, mode: u32) -> LocalFd {
    let cpath = std::ffi::CString::new(path.as_os_str().to_str().unwrap()).unwrap();
    // SAFETY: `cpath` is a valid NUL-terminated CStr built from a path with no
    // interior NUL; `libc::open` only reads the path plus the scalar flags and
    // mode and returns an fd number, dereferencing no pointer.
    let raw = unsafe { libc::open(cpath.as_ptr(), flags | libc::O_CLOEXEC, mode) };
    assert!(raw >= 0);
    // SAFETY: `raw` is a valid open fd just created above.
    unsafe { LocalFd::from_raw(raw) }
}

#[test]
fn copy_all_bytes() {
    let dir = Dir::new("all");
    let src = dir.path("src");
    std::fs::write(&src, b"hello world").unwrap();
    let dst = dir.path("dst");
    let in_fd = open(&src, libc::O_RDONLY, 0);
    let out_fd = open(&dst, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);

    let copied = sys::fileops::copy_file_range(&in_fd, &out_fd, 11).unwrap();
    assert_eq!(copied, 11);
    assert_eq!(std::fs::read(&dst).unwrap(), b"hello world");
}

#[test]
fn copy_partial_bytes() {
    let dir = Dir::new("partial");
    let src = dir.path("src");
    std::fs::write(&src, b"hello world").unwrap();
    let dst = dir.path("dst");
    let in_fd = open(&src, libc::O_RDONLY, 0);
    let out_fd = open(&dst, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);

    let copied = sys::fileops::copy_file_range(&in_fd, &out_fd, 5).unwrap();
    assert_eq!(copied, 5);
    assert_eq!(std::fs::read(&dst).unwrap(), b"hello");
}

#[test]
fn copy_zero_bytes() {
    let dir = Dir::new("zero");
    let src = dir.path("src");
    std::fs::write(&src, b"hello world").unwrap();
    let dst = dir.path("dst");
    let in_fd = open(&src, libc::O_RDONLY, 0);
    let out_fd = open(&dst, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);

    let copied = sys::fileops::copy_file_range(&in_fd, &out_fd, 0).unwrap();
    assert_eq!(copied, 0);
    assert_eq!(std::fs::read(&dst).unwrap(), b"");
}

#[test]
fn copy_more_than_available_returns_available() {
    let dir = Dir::new("eof");
    let src = dir.path("src");
    std::fs::write(&src, b"hello world").unwrap();
    let dst = dir.path("dst");
    let in_fd = open(&src, libc::O_RDONLY, 0);
    let out_fd = open(&dst, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);

    // The kernel caps the copy to the bytes available at `in_fd`.
    let copied = sys::fileops::copy_file_range(&in_fd, &out_fd, 1000).unwrap();
    assert_eq!(copied, 11);
    assert_eq!(std::fs::read(&dst).unwrap(), b"hello world");
}

#[test]
fn copy_bad_fd_errors() {
    // SAFETY: -1 is never a valid fd; copy_file_range returns EBADF.
    let bad = unsafe { LocalFd::from_raw(-1) };
    assert!(sys::fileops::copy_file_range(&bad, &bad, 10).is_err());
}
