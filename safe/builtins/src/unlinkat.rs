//! `unlinkat` builtin: remove a directory entry. Returns no fd.

use error_stack::{Report, ResultExt};
use sys::{AtFd, ImportedFd};

use crate::error::BuiltinError;

pub mod parse;

pub fn unlinkat_exec(cfg: &parse::UnlinkatConfig) -> Result<(), Report<BuiltinError>> {
    let dirfd = cfg.dirfd.as_ref().map_or(AtFd::cwd(), ImportedFd::at);
    sys::fileat::unlinkat(dirfd, cfg.path, cfg.flags).change_context(BuiltinError::Syscall)?;
    Ok(())
}
