use crate::error::BuiltinError;

pub mod parse;

pub fn fchmod_exec(cfg: &parse::FchmodConfig) -> Result<(), BuiltinError> {
    for &fd in &cfg.fds {
        sys::fchmod::fchmod(fd, cfg.mode)?;
    }
    Ok(())
}
