pub mod parse;

pub fn openat2_exec(cfg: &parse::Openat2Config) -> Result<(), i32> {
    let fd = sys::openat2::openat2(cfg.dirfd, cfg.path, &cfg.how)?;
    sys::shellfd::send_fd(fd, c"openat2")
}
