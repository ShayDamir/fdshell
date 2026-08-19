#![allow(clippy::unwrap_used)]

use std::os::unix::fs::PermissionsExt;

use sys::ImportedFd;

#[test]
fn fchmod_changes_mode() {
    let dir = std::env::temp_dir().join(format!("fdshell-fchmod-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
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
    sys::fchmod::fchmod(&fd, 0o640).unwrap();
    // SAFETY: `raw` is the valid open fd created above.
    unsafe { libc::close(raw) };

    // A stub returning Ok(()) without acting would leave the mode at 0o700.
    let st = sys::stat::stat(&cpath).unwrap();
    assert_eq!(st.mode & 0o777, 0o640);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fchmod_bad_fd_errors() {
    // SAFETY: -1 is never a valid fd; fchmod returns EBADF.
    let fd = unsafe { ImportedFd::from_raw(-1) };
    assert!(sys::fchmod::fchmod(&fd, 0o640).is_err());
}
