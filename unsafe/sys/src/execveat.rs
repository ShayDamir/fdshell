use alloc::vec::Vec;
use core::convert::AsRef;
use core::ffi::CStr;
use core::ptr;

use crate::AtFd;

#[cfg(coverage)]
unsafe extern "C" {
    fn __llvm_profile_write_file() -> i32;
}

pub use libc::AT_EMPTY_PATH;
pub use libc::AT_SYMLINK_NOFOLLOW;

pub fn execveat(
    dirfd: AtFd<'_>,
    pathname: &CStr,
    argv: &[impl AsRef<CStr>],
    envp: &[impl AsRef<CStr>],
    flags: i32,
) -> Result<(), i32> {
    let raw = dirfd.as_raw();
    let argv_ptrs: Vec<*const libc::c_char> = argv
        .iter()
        .map(|a| a.as_ref().as_ptr())
        .chain(core::iter::once(ptr::null()))
        .collect();
    let envp_ptrs: Vec<*const libc::c_char> = envp
        .iter()
        .map(|a| a.as_ref().as_ptr())
        .chain(core::iter::once(ptr::null()))
        .collect();
    #[cfg(coverage)]
    // SAFETY: single-threaded child after fork; LLVM's compiler-rt
    // provides __llvm_profile_write_file when -C instrument-coverage is used.
    unsafe {
        __llvm_profile_write_file();
    };
    // SAFETY: SYS_execveat (322) is valid on Linux ≥3.19 x86_64.
    // pathname, argv_ptrs, envp_ptrs point to valid memory. argv/envp
    // are NULL-terminated. dirfd is a valid fd or AT_FDCWD.
    crate::cvt(unsafe {
        libc::syscall(
            libc::SYS_execveat,
            raw as i64,
            pathname.as_ptr(),
            argv_ptrs.as_ptr(),
            envp_ptrs.as_ptr(),
            flags as i64,
        ) as isize
    })?;
    Ok(())
}
