#![allow(clippy::unwrap_used)]

use std::sync::atomic::AtomicU64;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn test_dir() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("fdshell-renameat2-{}-{}", std::process::id(), c))
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
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a"), b"x").unwrap();
    let dirfd = open_dir(&dir);
    sys::renameat2::renameat2(dirfd.at(), c"a", dirfd.at(), c"b", 0).unwrap();
    // A stub returning Ok(()) without acting would leave "a" and drop "b".
    assert!(!dir.join("a").exists());
    assert!(dir.join("b").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn renameat2_missing_source_errors() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let dirfd = open_dir(&dir);
    assert!(sys::renameat2::renameat2(dirfd.at(), c"nope", dirfd.at(), c"b", 0).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
