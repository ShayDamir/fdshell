#![no_std]

pub type Fd = i32;

pub(crate) fn cvt(ret: isize) -> Result<isize, i32> {
    if ret == -1 {
        // SAFETY: `__errno_location()` returns a valid pointer to thread-local errno,
        // guaranteed by the C runtime. Only called immediately after a failed libc call.
        unsafe { Err(*libc::__errno_location()) }
    } else {
        Ok(ret)
    }
}

pub mod fd;
pub mod net;
pub mod openat2;
mod process;
pub mod shellfd;
pub mod stat;
