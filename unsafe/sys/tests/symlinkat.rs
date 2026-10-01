#![allow(clippy::unwrap_used)]

use std::sync::atomic::AtomicU64;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn test_dir() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("fdshell-symlinkat-{}-{}", std::process::id(), c))
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
fn symlinkat_creates_link() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("target"), b"content").unwrap();
    let link = dir.join("link");

    let target = std::ffi::CString::new("target").unwrap();
    let newpath = std::ffi::CString::new("link").unwrap();
    let dirfd = open_dir(&dir);
    sys::fileat::symlinkat(&target, dirfd.at(), &newpath).unwrap();

    // A stub returning Ok(()) without acting would leave no link behind.
    assert_eq!(
        std::fs::read_link(&link).unwrap(),
        std::path::Path::new("target")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn symlinkat_eexist() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let dirfd = open_dir(&dir);
    let target = std::ffi::CString::new("t").unwrap();
    let newpath = std::ffi::CString::new("link").unwrap();
    sys::fileat::symlinkat(&target, dirfd.at(), &newpath).unwrap();

    let err = sys::fileat::symlinkat(&target, dirfd.at(), &newpath).unwrap_err();
    assert_eq!(err, sys::SyscallError::EEXIST("unknown"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn symlinkat_missing_parent_is_enoent() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let dirfd = open_dir(&dir);
    // `nope/link`: the parent `nope` does not exist.
    let target = std::ffi::CString::new("t").unwrap();
    let newpath = std::ffi::CString::new("nope/link").unwrap();
    let err = sys::fileat::symlinkat(&target, dirfd.at(), &newpath).unwrap_err();
    assert_eq!(err, sys::SyscallError::ENOENT("unknown"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn symlinkat_nondirectory_dirfd_is_enotdir() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("f");
    std::fs::write(&file, b"x").unwrap();
    let cfile = std::ffi::CString::new(file.to_str().unwrap()).unwrap();
    let fd = sys::openat2::open(cfile.as_c_str(), sys::fcntl::O_RDWR).unwrap();

    let target = std::ffi::CString::new("t").unwrap();
    let newpath = std::ffi::CString::new("link").unwrap();
    let err = sys::fileat::symlinkat(&target, fd.at(), &newpath).unwrap_err();
    assert_eq!(
        err,
        sys::SyscallError::Other {
            errno: libc::ENOTDIR,
            syscall: "unknown"
        }
    );
    drop(fd);
    let _ = std::fs::remove_dir_all(&dir);
}
