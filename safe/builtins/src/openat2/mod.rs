use sys::fcntl::O_CLOEXEC;

pub mod parse;

pub fn openat2_exec(cfg: &parse::Openat2Config) -> Result<(), i32> {
    let how = sys::openat2::OpenHow {
        flags: cfg.how.flags | (O_CLOEXEC as u64),
        mode: cfg.how.mode,
        resolve: cfg.how.resolve,
    };
    let fd = sys::openat2::openat2(cfg.dirfd, cfg.path, &how)?;
    sys::shellfd::send_fd(fd, c"openat2")
}
