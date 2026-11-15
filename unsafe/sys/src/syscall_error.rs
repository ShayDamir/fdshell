// mutants: -C --test -C syscall_error -- -E 'binary(syscall_error)'

//! Typed syscall error — bridges raw errno to the type system.
//!
//! `SyscallError` replaces raw `i32` errnos in `unsafe/sys/` return types so that
//! callers can use `error_stack::ResultExt::change_context()` instead of ad-hoc
//! `From<i32>` conversions.

use crate::errno::description::standard;
use crate::syscall_error_from::UNKNOWN;
use core::fmt;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum SyscallError {
    E2BIG(&'static str),
    EAGAIN(&'static str),
    EBADF(&'static str),
    EEXIST(&'static str),
    EINVAL(&'static str),
    EIO(&'static str),
    EMFILE(&'static str),
    ENOENT(&'static str),
    ENOSYS(&'static str),
    EPERM(&'static str),
    /// impossible state (indexing invariant violation)
    // Truly never: every producer is a bounds/clamp guard proven in place — the three
    // `net/getsockname.rs` arms, `net/addr.rs` behind its own `SUN_PATH_MAX`/`EINVAL`
    // gate, `fsverity.rs` where the buffer is the sha512 maximum, the partial-I/O
    // loops of `localfd/rw.rs` and `importedfd/rw.rs`, `env/getcwd.rs`,
    // `net/sendmsg.rs`'s unix-stream `sent == 0` invariant, and `localfd/reopen.rs`
    // (a decimal fd number carries no NUL). Pinned by `tests/syscall_error.rs`
    // (`display_never`, `display_renders_standard_descriptions`,
    // `from_errno_unknown`) and `tests/fsverity.rs::sha512_enable_measure_roundtrip`.
    // LESSONS: "A pid-derived test port is a shared name"
    // Chore #565: excluded from later respawns.
    Never,
    Other {
        errno: i32,
        syscall: &'static str,
    },
}

impl fmt::Display for SyscallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `Never` is an invariant violation, not an errno (STYLE 4.10), so it
        // answers before the table: its `errno()` is 0 and `standard(0)` is
        // deliberately `None`, which would render it through the fallback.
        if matches!(self, Self::Never) {
            return write!(f, "Never");
        }
        let (errno, syscall) = (self.errno(), self.syscall());
        match standard(errno) {
            Some(desc) if syscall == UNKNOWN => write!(f, "{desc}"),
            Some(desc) => write!(f, "{desc} ({syscall})"),
            None => write!(f, "{syscall} (errno {errno})"),
        }
    }
}

impl SyscallError {
    pub fn errno(self) -> i32 {
        use SyscallError::*;
        match self {
            E2BIG(_) => libc::E2BIG,
            EAGAIN(_) => libc::EAGAIN,
            EBADF(_) => libc::EBADF,
            EEXIST(_) => libc::EEXIST,
            EINVAL(_) => libc::EINVAL,
            EIO(_) => libc::EIO,
            EMFILE(_) => libc::EMFILE,
            ENOENT(_) => libc::ENOENT,
            ENOSYS(_) => libc::ENOSYS,
            EPERM(_) => libc::EPERM,
            Never => 0,
            Other { errno, .. } => errno,
        }
    }

    pub fn syscall(self) -> &'static str {
        use SyscallError::*;
        match self {
            E2BIG(s) | EAGAIN(s) | EBADF(s) | EEXIST(s) | EINVAL(s) | EIO(s) | EMFILE(s)
            | ENOENT(s) | ENOSYS(s) | EPERM(s) => s,
            Never => "Never",
            Other { syscall, .. } => syscall,
        }
    }
}

impl core::error::Error for SyscallError {}
