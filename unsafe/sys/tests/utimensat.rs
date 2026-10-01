#![allow(clippy::unwrap_used)]

use std::sync::atomic::AtomicU64;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn test_dir() -> std::path::PathBuf {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("fdshell-utimensat-{}-{}", std::process::id(), c))
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

/// mtime in epoch seconds, following symlinks (stat).
fn mtime_secs(path: &std::path::Path) -> i64 {
    let st = std::fs::metadata(path).unwrap().modified().unwrap();
    st.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

/// atime in epoch seconds, following symlinks (stat).
fn atime_secs(path: &std::path::Path) -> i64 {
    let st = std::fs::metadata(path).unwrap().accessed().unwrap();
    st.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

/// mtime in epoch seconds of the link itself (lstat).
fn lmtime_secs(path: &std::path::Path) -> i64 {
    let st = std::fs::symlink_metadata(path).unwrap().modified().unwrap();
    st.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

/// Stat calls do not bump atime (relatime), so these helpers are safe to mix.
fn epoch(sec: i64) -> sys::fileat::Timespec {
    sys::fileat::Timespec {
        tv_sec: sec,
        tv_nsec: 0,
    }
}

const NOW: sys::fileat::Timespec = sys::fileat::Timespec {
    tv_sec: 0,
    tv_nsec: sys::fileat::UTIME_NOW,
};
const OMIT: sys::fileat::Timespec = sys::fileat::Timespec {
    tv_sec: 0,
    tv_nsec: sys::fileat::UTIME_OMIT,
};

#[test]
fn utimensat_sets_epoch() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f"), b"x").unwrap();
    let dirfd = open_dir(&dir);
    let path = std::ffi::CString::new("f").unwrap();
    sys::fileat::utimensat(dirfd.at(), &path, &epoch(1234567890), &epoch(1234567890), 0).unwrap();
    assert_eq!(mtime_secs(&dir.join("f")), 1234567890);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn utimensat_now_is_current() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f"), b"x").unwrap();
    let dirfd = open_dir(&dir);
    let path = std::ffi::CString::new("f").unwrap();
    sys::fileat::utimensat(dirfd.at(), &path, &NOW, &NOW, 0).unwrap();
    let m = mtime_secs(&dir.join("f"));
    let real = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    assert!(
        (m - real).abs() <= 5,
        "mtime {m} not within 5s of now {real}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn utimensat_omit_leaves_mtime() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f"), b"x").unwrap();
    let dirfd = open_dir(&dir);
    let path = std::ffi::CString::new("f").unwrap();
    sys::fileat::utimensat(dirfd.at(), &path, &epoch(1111111111), &epoch(1111111111), 0).unwrap();
    assert_eq!(mtime_secs(&dir.join("f")), 1111111111);
    assert_eq!(atime_secs(&dir.join("f")), 1111111111);
    // atime -> now, mtime -> omit: mtime stays, atime moves.
    sys::fileat::utimensat(dirfd.at(), &path, &NOW, &OMIT, 0).unwrap();
    assert_eq!(
        mtime_secs(&dir.join("f")),
        1111111111,
        "mtime must be untouched by UTIME_OMIT"
    );
    assert_ne!(
        atime_secs(&dir.join("f")),
        1111111111,
        "atime must move to now"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn utimensat_nofollow_updates_link_not_target() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("target");
    std::fs::write(&target, b"x").unwrap();
    let link = dir.join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let dirfd = open_dir(&dir);
    // Pin the target's mtime to a distinct value so the two can be told apart.
    let tpath = std::ffi::CString::new("target").unwrap();
    sys::fileat::utimensat(
        dirfd.at(),
        &tpath,
        &epoch(2222222222),
        &epoch(2222222222),
        0,
    )
    .unwrap();
    // Update the link's own timestamps, not the target's.
    let lpath = std::ffi::CString::new("link").unwrap();
    sys::fileat::utimensat(
        dirfd.at(),
        &lpath,
        &epoch(1234567890),
        &epoch(1234567890),
        sys::fileat::AT_SYMLINK_NOFOLLOW,
    )
    .unwrap();
    assert_eq!(lmtime_secs(&link), 1234567890, "link mtime must be set");
    assert_eq!(
        mtime_secs(&target),
        2222222222,
        "target mtime must be untouched"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn utimensat_enoent() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let dirfd = open_dir(&dir);
    let path = std::ffi::CString::new("nope").unwrap();
    let err = sys::fileat::utimensat(dirfd.at(), &path, &NOW, &NOW, 0).unwrap_err();
    assert_eq!(err, sys::SyscallError::ENOENT("unknown"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn utimensat_nondirectory_dirfd_is_enotdir() {
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("f");
    std::fs::write(&file, b"x").unwrap();
    let cfile = std::ffi::CString::new(file.to_str().unwrap()).unwrap();
    let fd = sys::openat2::open(cfile.as_c_str(), sys::fcntl::O_RDWR).unwrap();
    let path = std::ffi::CString::new("f").unwrap();
    let err = sys::fileat::utimensat(fd.at(), &path, &NOW, &NOW, 0).unwrap_err();
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
