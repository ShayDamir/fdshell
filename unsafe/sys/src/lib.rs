#![no_std]

extern crate alloc;

pub use atfd::AtFd;
pub use cmdline::ReadCmdlineError;
pub use exportedfd::ExportedFd;
pub use importedfd::ImportedFd;
pub use importedfd_error::ImportedFdError;
pub use importedstr::{ImportedStr, Origin, Position, ScriptText, Trace};
pub use localfd::LocalFd;
pub use localfd_error::LocalFdError;
pub use pid::Pid;
pub use shellfd::RecvFdError;
pub use shortcstr::{ExportedCStr, NoNul, ShortCStr, ShortCStrError};
pub use syscall_error::SyscallError;
pub use umask::UmaskError;

pub use exit::exit;

pub fn cvt(ret: isize) -> Result<isize, SyscallError> {
    (ret != -1).then_some(ret).ok_or_else(errno_err)
}

/// Like [`cvt`], but for libc calls that return a 64-bit value (e.g. `off_t`).
pub fn cvt64(ret: i64) -> Result<i64, SyscallError> {
    (ret != -1).then_some(ret).ok_or_else(errno_err)
}

/// The errno of the last failed libc call.
fn errno_err() -> SyscallError {
    // SAFETY: `__errno_location()` returns a valid pointer to thread-local errno,
    // guaranteed by the C runtime. Only called immediately after a failed libc call.
    unsafe { (*libc::__errno_location()).into() }
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

pub mod access;
pub mod atfd;
pub mod cmdline;
pub mod dup;
pub mod env;
pub mod errno;
pub mod eventfd;
pub mod execveat;
mod exit;
pub mod exportedfd;
pub mod fcntl;
pub mod ficlone;
pub mod fileat;
pub mod fileops;
pub mod fork_cell;
pub mod fork_pidfd;
pub mod fsverity;
pub mod getdents64;
pub mod getrusage;
pub mod importedfd;
pub mod importedfd_error;
pub mod importedfd_try;
pub mod importedstr;
pub mod localfd;
pub mod localfd_error;
pub mod memfd;
pub mod net;
pub mod openat2;
pub mod pid;
pub mod pipe;
pub mod poll;
pub mod pty;
pub mod rlimit;
pub mod shellfd;
pub mod shortcstr;
pub mod siginfo;
pub mod signal;
pub mod signalfd;
pub mod split;
pub mod stat;
pub mod statx;
pub mod syscall_error;
pub mod syscall_error_from;
pub mod timerfd;
pub mod umask;
