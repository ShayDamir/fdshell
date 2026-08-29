use crate::LocalFd;

pub const S_IFMT: u32 = libc::S_IFMT;
pub const S_IFSOCK: u32 = libc::S_IFSOCK;
pub const S_IFLNK: u32 = libc::S_IFLNK;
pub const S_IFREG: u32 = libc::S_IFREG;
pub const S_IFBLK: u32 = libc::S_IFBLK;
pub const S_IFDIR: u32 = libc::S_IFDIR;
pub const S_IFCHR: u32 = libc::S_IFCHR;
pub const S_IFIFO: u32 = libc::S_IFIFO;
pub const S_ISGID: u32 = libc::S_ISGID;
pub const S_ISVTX: u32 = libc::S_ISVTX;

// `PartialEq` + `Debug` are derived plainly (not `#[cfg(test)]`) because the
// cross-crate integration test `safe/builtins/tests/openat2.rs` asserts
// `assert_eq!(before, after)` while `sys` is compiled as a dependency (no test
// cfg). All fields are primitives, so no trait-propagation concern.
#[derive(PartialEq, Debug)]
pub struct FileStat {
    pub ino: u64,
    pub mode: u32,
    pub dev: u64,
    pub rdev: u64,
    pub size: u64,
    pub mtime: i64,
}

fn to_filestat(raw: &libc::stat) -> FileStat {
    FileStat {
        ino: raw.st_ino,
        mode: raw.st_mode,
        dev: raw.st_dev,
        rdev: raw.st_rdev,
        size: raw.st_size as u64,
        mtime: raw.st_mtime,
    }
}

pub fn fstat(fd: &LocalFd) -> Result<FileStat, crate::SyscallError> {
    // SAFETY: zero-initialized `libc::stat` is valid (all integer fields).
    let mut raw: libc::stat = unsafe { core::mem::zeroed() };
    // SAFETY: `fd.as_raw()` is a valid fd by the `LocalFd` invariant; `fstat`
    // on an invalid fd returns `EBADF`, caught by `cvt`.
    crate::cvt(unsafe { libc::fstat(fd.as_raw(), &mut raw) as isize })?;
    Ok(to_filestat(&raw))
}

pub fn stat(path: &core::ffi::CStr) -> Result<FileStat, crate::SyscallError> {
    // SAFETY: zero-initialized `libc::stat` is valid (all integer fields).
    let mut raw: libc::stat = unsafe { core::mem::zeroed() };
    // SAFETY: `path` is a valid null-terminated C string; `stat` on a bad path
    // returns `ENOENT`/`ENOTDIR`, caught by `cvt`.
    crate::cvt(unsafe { libc::stat(path.as_ptr(), &mut raw) as isize })?;
    Ok(to_filestat(&raw))
}

pub fn lstat(path: &core::ffi::CStr) -> Result<FileStat, crate::SyscallError> {
    // SAFETY: zero-initialized `libc::stat` is valid (all integer fields).
    let mut raw: libc::stat = unsafe { core::mem::zeroed() };
    // SAFETY: `path` is a valid null-terminated C string; `lstat` on a bad
    // path returns `ENOENT`, caught by `cvt`.
    crate::cvt(unsafe { libc::lstat(path.as_ptr(), &mut raw) as isize })?;
    Ok(to_filestat(&raw))
}
