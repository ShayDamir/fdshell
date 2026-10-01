//! `symlinkat` builtin: create a symbolic link. Returns no fd.

use error_stack::{Report, ResultExt};
use sys::{AtFd, ImportedFd};

use crate::error::BuiltinError;

pub mod parse;

pub fn symlinkat_exec(cfg: &parse::SymlinkatConfig) -> Result<(), Report<BuiltinError>> {
    let dirfd = cfg.dirfd.as_ref().map_or(AtFd::cwd(), ImportedFd::at);
    sys::fileat::symlinkat(cfg.target, dirfd, cfg.linkpath)
        .change_context(BuiltinError::Syscall)?;
    Ok(())
}
