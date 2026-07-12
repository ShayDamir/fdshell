#![no_std]

extern crate alloc;

pub use atfd::AtFd;
pub use exportedfd::ExportedFd;
pub use importedfd::ImportedFd;
pub use importedfd_error::ImportedFdError;
pub use localfd::LocalFd;
pub use recv_fd_error::RecvFdError;
pub use shortcstr::{RefCStr, ShortCStr, ShortCStrError};
pub use syscall_error::SyscallError;
pub use umask::UmaskError;

pub fn cvt(ret: isize) -> Result<isize, SyscallError> {
    if ret == -1 {
        // SAFETY: `__errno_location()` returns a valid pointer to thread-local errno,
        // guaranteed by the C runtime. Only called immediately after a failed libc call.
        unsafe { Err((*libc::__errno_location()).into()) }
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
pub mod importedfd_error;
pub mod importedfd_try;
pub mod iovec;
pub mod localfd;
pub mod mkdirat;
pub mod net;
pub mod openat2;
pub mod pipe;
pub mod recv_fd_error;
pub mod renameat2;
pub mod rw;
pub mod shellfd;
pub mod shortcstr;
pub mod siginfo;
pub mod split;
pub mod stat;
pub mod syscall_error;
pub mod syscall_error_from;
pub mod umask;
pub mod unlinkat;
pub mod wait_pidfd;
