#![no_std]

extern crate alloc;

pub use atfd::AtFd;
pub use exportedfd::ExportedFd;
pub use importedfd::ImportedFd;
pub use localfd::LocalFd;
pub use shortcstr::{RefCStr, ShortCStr};

pub fn cvt(ret: isize) -> Result<isize, i32> {
    if ret == -1 {
        // SAFETY: `__errno_location()` returns a valid pointer to thread-local errno,
        // guaranteed by the C runtime. Only called immediately after a failed libc call.
        unsafe { Err(*libc::__errno_location()) }
    } else {
        Ok(ret)
    }
}

pub mod atfd;
pub mod errno;
pub mod execveat;
pub mod exportedfd;
pub mod fchdir;
pub mod fchmod;
pub mod fcntl;
pub mod fork_cell;
pub mod fork_pidfd;
pub mod importedfd;
pub mod iovec;
pub mod localfd;
pub mod mkdirat;
pub mod net;
pub mod openat2;
pub mod pipe;
pub mod renameat2;
pub mod rw;
pub mod shellfd;
pub mod shortcstr;
pub mod siginfo;
pub mod stat;
pub mod umask;
pub mod unlinkat;
pub mod wait_pidfd;
