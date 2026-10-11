//! `--mode` policy for the create builtins (`mkdirat`, `mkfifoat`, `openat2`).
//!
//! Every create builtin ends with a follow-up `openat2` that reopens the object
//! it just created, and the kernel stores the requested mode as `mode & ~umask`.
//! A mode that this reduction leaves without the owner bits the reopen needs
//! yields `EACCES` *after* the object is on disk with no fd captured. The
//! constants are the coreutils defaults; `require_reopenable` rejects a request
//! whose masked value cannot support the reopen, before any syscall runs.

use error_stack::{Report, ensure};
use sys::umask;

use crate::error::BuiltinError;

/// `mkdirat`'s default mode: `mkdir(1)`'s `0777` (the kernel applies the umask).
/// A directory needs the owner `x` bit for path lookups through the fd it exports.
pub const DIR_MODE: u32 = 0o777;

/// `mkfifoat`'s and `openat2 --flags O_CREAT`'s default mode: `mkfifo(1)` and
/// POSIX `open(O_CREAT)`'s `0666` (the kernel applies the umask).
pub const CREATE_MODE: u32 = 0o666;

/// Owner bits the `mkdirat` reopen (`O_RDONLY`) needs on the created directory.
pub const DIR_REOPEN: u32 = 0o500;

/// Owner bits the `mkfifoat` reopen (`O_RDWR`) needs on the created FIFO.
pub const FIFO_REOPEN: u32 = 0o600;

/// Reject a mode whose value after the umask — the kernel's `mode & ~umask` —
/// lacks the owner bits `needed` for the builtin's own reopen. Runs before any
/// syscall, so an unusable mode creates nothing.
pub fn require_reopenable(requested: u32, needed: u32) -> Result<(), Report<BuiltinError>> {
    ensure!(
        (requested & !umask::get()) & needed == needed,
        BuiltinError::ModeReopenDenied
    );
    Ok(())
}

#[cfg(test)]
mod tests;
