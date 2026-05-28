use sys::fcntl::{O_CLOEXEC, O_DIRECTORY, O_NOFOLLOW};
use sys::{AtFd, ImportedFd};

pub mod parse;

pub fn mkdirat_exec(cfg: &parse::MkdiratConfig) -> Result<(), i32> {
    let dirfd = cfg.dirfd.as_ref().map_or(AtFd::cwd(), ImportedFd::at);
    sys::mkdirat::mkdirat(dirfd, cfg.path, cfg.mode & 0o777)?;
    let how = sys::openat2::OpenHow {
        flags: (O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW) as u64,
        mode: 0,
        resolve: cfg.resolve,
    };
    let fd = sys::openat2::openat2(dirfd, cfg.path, &how)?;
    sys::shellfd::send_fd(&fd, c"dirfd").ok();
    Ok(())
}
