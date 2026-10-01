//! Strict (capability) mode guards.
//!
//! When the shell's `strict` option is on, the `*at` builtins ban absolute
//! path resolution: every operation must be relative to an explicit `--dirfd`.
//! The two bans are (1) an omitted / `AT_FDCWD` dirfd, which resolves against
//! the process CWD, and (2) an absolute path (leading `/`), which the kernel
//! resolves from the filesystem root regardless of `--dirfd`. This module
//! holds the two guards shared by the `*at` parse functions.

use core::ffi::CStr;
use error_stack::{Report, bail};
use sys::ImportedFd;

use crate::error::BuiltinError;

/// Reject an omitted / `AT_FDCWD` dirfd while strict mode is on.
pub fn require_dirfd(strict: bool, dirfd: Option<&ImportedFd>) -> Result<(), Report<BuiltinError>> {
    if strict && dirfd.is_none() {
        bail!(BuiltinError::StrictRequiresDirfd);
    }
    Ok(())
}

/// Reject an absolute path (leading `/`) while strict mode is on.
pub fn require_relative(strict: bool, path: &CStr) -> Result<(), Report<BuiltinError>> {
    if strict && path.to_bytes().starts_with(b"/") {
        bail!(BuiltinError::StrictAbsolutePath);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
