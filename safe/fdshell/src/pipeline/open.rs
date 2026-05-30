#![forbid(unsafe_code)]

use crate::parse::CommandLine;
use crate::redirect::RedirectDirection::*;
use crate::redirect::RedirectSource::*;
use sys::LocalFd;

pub fn open_redirect_files(cmd_data: &CommandLine) -> Vec<LocalFd> {
    let mut opened: Vec<LocalFd> = Vec::with_capacity(cmd_data.redirects.len());
    for r in &cmd_data.redirects {
        if let Path(path) = &r.source {
            let flags = match r.direction {
                Read => sys::fcntl::O_RDONLY,
                Write => sys::fcntl::O_WRONLY | sys::fcntl::O_CREAT | sys::fcntl::O_TRUNC,
            };
            let name = path.to_c_string();
            let fd = match sys::openat2::openat2(
                sys::atfd::AtFd::cwd(),
                &name,
                &sys::openat2::OpenHow::new(flags as u64 | sys::fcntl::O_CLOEXEC as u64, 0o666),
            ) {
                Ok(f) => f,
                Err(e) => std::process::exit(e),
            };
            opened.push(fd);
        }
    }
    opened
}
