use crate::Fd;

pub struct FileStat {
    pub ino: u64,
    pub mode: u32,
    pub dev: u64,
    pub rdev: u64,
}

impl PartialEq for FileStat {
    fn eq(&self, other: &Self) -> bool {
        self.ino == other.ino
            && self.mode == other.mode
            && self.dev == other.dev
            && self.rdev == other.rdev
    }
}

impl core::fmt::Debug for FileStat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FileStat")
            .field("ino", &self.ino)
            .field("mode", &self.mode)
            .field("dev", &self.dev)
            .field("rdev", &self.rdev)
            .finish()
    }
}

pub fn fstat(fd: &Fd) -> Result<FileStat, i32> {
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
