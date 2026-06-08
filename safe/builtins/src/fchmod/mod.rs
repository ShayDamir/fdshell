pub mod parse;

pub fn fchmod_exec(cfg: &parse::FchmodConfig) -> Result<(), i32> {
    for &fd in &cfg.fds {
        sys::fchmod::fchmod(fd, cfg.mode)?;
    }
    Ok(())
}
