use crate::LocalFd;

pub const S_IFMT: u32 = libc::S_IFMT;
pub const S_IFDIR: u32 = libc::S_IFDIR;

#[derive(PartialEq, Debug)]
pub struct FileStat {
    pub ino: u64,
    pub mode: u32,
    pub dev: u64,
    pub rdev: u64,
}

pub fn fstat(fd: &LocalFd) -> Result<FileStat, i32> {
    // SAFETY: `fd.as_raw()` is any integer; `libc::fstat` with an invalid fd returns `EBADF`.
    // `raw` is zero-initialized, valid for a `libc::stat` (all integer fields).
    let mut raw: libc::stat = unsafe { core::mem::zeroed() };
    crate::cvt(unsafe { libc::fstat(fd.as_raw(), &mut raw) as isize })?;
    Ok(FileStat {
        ino: raw.st_ino as u64,
        mode: raw.st_mode as u32,
        dev: raw.st_dev as u64,
        rdev: raw.st_rdev as u64,
    })
}

pub fn stat(path: &core::ffi::CStr) -> Result<FileStat, i32> {
    // SAFETY: `path` must be a valid null-terminated C string; an invalid path
    // returns `ENOENT`/`ENOTDIR`. `raw` is zero-initialized, valid for `libc::stat`.
    let mut raw: libc::stat = unsafe { core::mem::zeroed() };
    crate::cvt(unsafe { libc::stat(path.as_ptr(), &mut raw) as isize })?;
    Ok(FileStat {
        ino: raw.st_ino as u64,
        mode: raw.st_mode as u32,
        dev: raw.st_dev as u64,
        rdev: raw.st_rdev as u64,
    })
}
