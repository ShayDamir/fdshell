#![no_std]

extern crate alloc;

pub use atfd::AtFd;
pub use cmdline::ReadCmdlineError;
pub use exportedfd::ExportedFd;
pub use importedfd::ImportedFd;
pub use importedfd_error::ImportedFdError;
pub use importedfd_io::ImportedFdIo;
pub use localfd::LocalFd;
pub use localfd_error::LocalFdError;
pub use recv_fd_error::RecvFdError;
pub use shortcstr::{RefCStr, ShortCStr, ShortCStrError};
pub use syscall_error::SyscallError;
pub use umask::UmaskError;

pub use exit::exit;

pub fn cvt(ret: isize) -> Result<isize, SyscallError> {
    if ret == -1 {
        // SAFETY: `__errno_location()` returns a valid pointer to thread-local errno,
        // guaranteed by the C runtime. Only called immediately after a failed libc call.
        unsafe { Err((*libc::__errno_location()).into()) }
    } else {
        Ok(ret)
    }
}

/// Helper to create static ImportedFd instances from raw fds.
///
/// # Safety
/// The fd must be a valid open fd with CLOEXEC clear (fds 0/1/2 always satisfy this).
const fn std_fd(fd: i32) -> ImportedFd {
    // SAFETY: fds 0/1/2 are always valid and have CLOEXEC clear in any POSIX process.
    unsafe { ImportedFd::from_raw(fd) }
}

/// Standard input (fd 0).
pub static IN: ImportedFd = std_fd(0);
/// Standard output (fd 1).
pub static OUT: ImportedFd = std_fd(1);
/// Standard error (fd 2).
pub static ERR: ImportedFd = std_fd(2);

pub mod atfd;
pub mod cmdline;
pub mod env;
pub mod errno;
pub mod execveat;
mod exit;
pub mod exportedfd;
pub mod fchdir;
pub mod fchmod;
pub mod fcntl;
pub mod fork_cell;
pub mod fork_pidfd;
pub mod importedfd;
pub mod importedfd_error;
pub mod importedfd_io;
pub mod importedfd_try;
pub mod iovec;
pub mod localfd;
pub mod localfd_error;
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
