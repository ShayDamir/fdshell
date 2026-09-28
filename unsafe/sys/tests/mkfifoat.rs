#![allow(clippy::unwrap_used)]

use std::sync::atomic::AtomicU64;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn test_dir() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("fdshell-mkfifoat-{}-{}", std::process::id(), c))
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
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let dirfd = open_dir(&dir);
    sys::fileat::mkfifoat(dirfd.at(), c"pipe", 0o600).unwrap();
    // A stub returning Ok(()) without acting would leave the fifo absent.
    let st = sys::stat::stat(&std::ffi::CString::new(dir.join("pipe").to_str().unwrap()).unwrap())
        .unwrap();
    assert_eq!(st.mode & sys::stat::S_IFMT, sys::stat::S_IFIFO);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mkfifoat_existing_path_is_eexist() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::File::create(dir.join("pipe")).unwrap();
    let dirfd = open_dir(&dir);
    let err = sys::fileat::mkfifoat(dirfd.at(), c"pipe", 0o600).unwrap_err();
    assert_eq!(err.errno(), libc::EEXIST);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mkfifoat_missing_parent_is_enoent() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let dirfd = open_dir(&dir);
    let err = sys::fileat::mkfifoat(dirfd.at(), c"no/such/pipe", 0o600).unwrap_err();
    assert_eq!(err.errno(), libc::ENOENT);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mkfifoat_regular_file_dirfd_is_enotdir() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f"), b"x").unwrap();
    let cfile = std::ffi::CString::new(dir.join("f").to_str().unwrap()).unwrap();
    // SAFETY: `cfile` is a valid NUL-terminated path.
    let reg = unsafe { libc::open(cfile.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    assert!(reg >= 0);
    // SAFETY: `reg` is a valid open fd with CLOEXEC set.
    let regfd = unsafe { sys::LocalFd::from_raw(reg) };
    let err = sys::fileat::mkfifoat(regfd.at(), c"pipe", 0o600).unwrap_err();
    assert_eq!(err.errno(), libc::ENOTDIR);
    let _ = std::fs::remove_dir_all(&dir);
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
