#![forbid(unsafe_code)]

use crate::error::redirect::OpenRedirectError;
use crate::parse::CommandLine;
use error_stack::Report;
use sys::LocalFd;

pub fn open_redirect_files(
    cmd_data: &CommandLine,
) -> Result<Vec<LocalFd>, Report<OpenRedirectError>> {
    crate::redirect::open_redirect_files(&cmd_data.redirects)
}
