#![forbid(unsafe_code)]

use crate::parse::CommandLine;
use crate::redirect::RedirectSource::*;
use sys::LocalFd;
use sys::fcntl::{O_CLOEXEC, O_CREAT};

pub fn open_redirect_files(cmd_data: &CommandLine) -> Vec<LocalFd> {
    let mut opened: Vec<LocalFd> = Vec::with_capacity(cmd_data.redirects.len());
    for r in &cmd_data.redirects {
        if let Path(path) = &r.source {
            let flags = r.direction.open_flags();
            let name = path.to_c_string();
            let fd = match sys::openat2::openat2(
                sys::atfd::AtFd::cwd(),
                &name,
                &sys::openat2::OpenHow::new(
                    (flags | O_CLOEXEC) as u64,
                    if flags & O_CREAT != 0 { 0o666 } else { 0 },
                ),
            ) {
                Ok(f) => f,
                Err(e) => std::process::exit(e),
            };
            opened.push(fd);
        }
    }
    opened
}
