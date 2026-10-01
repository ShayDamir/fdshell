//! `utimensat` builtin: set atime/mtime. Returns no fd.

use error_stack::{Report, ResultExt};
use sys::{AtFd, ImportedFd};

use crate::error::BuiltinError;

pub mod parse;

pub fn utimensat_exec(cfg: &parse::UtimensatConfig) -> Result<(), Report<BuiltinError>> {
    let dirfd = cfg.dirfd.as_ref().map_or(AtFd::cwd(), ImportedFd::at);
    let atime = cfg.atime.to_timespec();
    let mtime = cfg.mtime.to_timespec();
    sys::fileat::utimensat(dirfd, cfg.path, &atime, &mtime, cfg.flags)
        .change_context(BuiltinError::Syscall)?;
    Ok(())
}
