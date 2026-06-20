#![forbid(unsafe_code)]

use crate::parse::CommandLine;
use sys::LocalFd;

pub fn open_redirect_files(cmd_data: &CommandLine) -> Vec<LocalFd> {
    match crate::redirect::open_redirect_files(&cmd_data.redirects) {
        Ok(fds) => fds,
        Err(_) => std::process::exit(1),
    }
}
