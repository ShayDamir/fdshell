use error_stack::{Report, ResultExt};
use sys::AtFd;
use sys::ImportedFd;

use crate::error::BuiltinError;

pub mod parse;

pub fn renameat2_exec(cfg: &parse::Renameat2Config) -> Result<(), Report<BuiltinError>> {
    let olddirfd = cfg.olddirfd.as_ref().map_or(AtFd::cwd(), ImportedFd::at);
    let newdirfd = cfg.newdirfd.as_ref().map_or(AtFd::cwd(), ImportedFd::at);
    sys::renameat2::renameat2(olddirfd, cfg.oldpath, newdirfd, cfg.newpath, cfg.flags)
        .change_context(BuiltinError::Syscall)?;
    Ok(())
}
