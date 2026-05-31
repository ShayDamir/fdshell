use crate::fcntl::{O_CLOEXEC, O_CREAT};
use crate::{AtFd, LocalFd};
use core::ffi::CStr;

#[repr(C)]
pub struct OpenHow {
    pub flags: u64,
    pub mode: u64,
    pub resolve: u64,
}

impl OpenHow {
    pub fn new(flags: u64, mode: u64) -> Self {
        OpenHow {
            flags,
            mode,
            resolve: 0,
        }
    }
}

pub const RESOLVE_NO_SYMLINKS: u64 = 1;
pub const RESOLVE_NO_MAGICLINKS: u64 = 2;
pub const RESOLVE_NO_XDEV: u64 = 4;
pub const RESOLVE_BENEATH: u64 = 8;
pub const RESOLVE_IN_ROOT: u64 = 16;
pub const RESOLVE_CACHED: u64 = 32;

pub fn open(pathname: &CStr, flags: i32) -> Result<LocalFd, i32> {
    let mode = if flags & O_CREAT != 0 { 0o666 } else { 0 };
    openat2(
        AtFd::cwd(),
        pathname,
        &OpenHow::new((flags | O_CLOEXEC) as u64, mode),
    )
}

pub fn openat2(dirfd: AtFd<'_>, pathname: &CStr, how: &OpenHow) -> Result<LocalFd, i32> {
    let dirfd = dirfd.as_raw();
    // SAFETY: SYS_openat2 (437) is valid on Linux ≥5.6 x86_64. dirfd may be
    // AT_FDCWD (−100) or an open dirfd. pathname and how point to valid memory
    // and are only read by the kernel.
    crate::cvt(unsafe {
        libc::syscall(
            libc::SYS_openat2,
            dirfd as i64,
            pathname.as_ptr(),
            &raw const *how,
            core::mem::size_of_val(how),
        ) as isize
    })
    .map(|ret| {
        // SAFETY: `ret` is a valid fd with CLOEXEC (ORed into `how.flags` by the caller).
        unsafe { LocalFd::from_raw(ret as i32) }
    })
}
