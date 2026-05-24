#![no_std]

pub use atfd::AtFd;
pub use fd::{DupFd, Fd};

pub(crate) fn cvt(ret: isize) -> Result<isize, i32> {
    if ret == -1 {
        // SAFETY: `__errno_location()` returns a valid pointer to thread-local errno,
        // guaranteed by the C runtime. Only called immediately after a failed libc call.
        unsafe { Err(*libc::__errno_location()) }
    } else {
        Ok(ret)
    }
}

pub mod atfd;
pub mod fcntl;
pub mod fd;
pub mod mkdirat;
pub mod net;
pub mod openat2;
pub mod pipe;
mod process;
pub mod renameat;
pub mod rw;
pub mod shellfd;
pub mod stat;
