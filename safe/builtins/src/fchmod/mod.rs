use error_stack::{Report, ResultExt};

use crate::error::BuiltinError;

pub mod parse;

pub fn fchmod_exec(cfg: &parse::FchmodConfig) -> Result<(), Report<BuiltinError>> {
    for fd in &cfg.fds {
        sys::fileops::fchmod(fd, cfg.mode).change_context(BuiltinError::Syscall)?;
    }
    Ok(())
}
