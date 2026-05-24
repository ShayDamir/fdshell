use sys::AtFd;
use sys::DupFd;

pub mod parse;

pub fn renameat2_exec(cfg: &parse::Renameat2Config) -> Result<(), i32> {
    let olddirfd = cfg.olddirfd.as_ref().map_or(AtFd::cwd(), DupFd::at);
    let newdirfd = cfg.newdirfd.as_ref().map_or(AtFd::cwd(), DupFd::at);
    sys::renameat2::renameat2(olddirfd, cfg.oldpath, newdirfd, cfg.newpath, cfg.flags)
}
