use core::fmt::Write;
use error_stack::{Report, ResultExt};

use crate::LocalFd;
use crate::ShortCStr;
use crate::SyscallError;
use crate::fcntl::O_RDONLY;

impl LocalFd {
    /// Re-open this descriptor **read-only**, as a fresh open file description.
    ///
    /// `dup`/`dup2`/`fcntl(F_DUPFD)` copy a descriptor *inside* one open file
    /// description, and the access mode is a property of the description, not of
    /// the descriptor — no clone can make a read/write fd refuse writes. The
    /// `/proc/self/fd/N` magic link is the one path that changes the mode.
    /// Measured on Linux 7.3.0, on a memfd:
    ///
    /// - the result is a **new open file description**, so its offset starts at
    ///   `0` independently of the source fd's offset (which is then never
    ///   observable — nothing has to seek the source first);
    /// - `write` on the new fd fails `EBADF` while `read` returns the bytes, and
    ///   so does `write` on any `dup` of it: the mode travels with the
    ///   description, which is what makes a here-doc/here-string source refuse
    ///   `>&N` writes with bash's `Bad file descriptor`;
    /// - reopening through `/proc` does **not** restrict the mode by itself —
    ///   `O_WRONLY` on the same memfd succeeds — so the caller chooses the mode
    ///   here. Sealing is no substitute: `F_SEAL_WRITE` makes writes fail
    ///   `EPERM`, not `EBADF`, so the diagnostic stops matching bash.
    ///
    /// The mode is deliberately not a parameter: the point of the wrapper is
    /// that no caller can request a writable re-open. `O_RDONLY` goes in alone —
    /// it is `0`, so putting it in a flag expression only makes the operator
    /// unkillable (LESSONS: "An `O_RDONLY` (== 0) flag is inert and makes a `|`
    /// flag expression unkillable").
    ///
    /// The caller inherits a `/proc` runtime dependency.
    pub fn reopen_read_only(&self) -> Result<LocalFd, Report<SyscallError>> {
        // The path is the `/proc/self/fd/N` magic link. A decimal fd number
        // carries no NUL byte, so `fmt::Write`'s failure arm is the impossible
        // case (STYLE §4.10, and `ShortCStr`'s `fmt::Write` discards the
        // `ShortCStrError` for exactly that reason).
        let mut path = ShortCStr::new();
        path.write_fmt(format_args!("/proc/self/fd/{self}"))
            .change_context(SyscallError::Never)?;
        // `openat2::open` forces `O_CLOEXEC`, the `LocalFd` invariant (STYLE §5.2).
        Ok(crate::openat2::open(path.export(), O_RDONLY)?)
    }
}
