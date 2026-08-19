use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::{AtFd, ExportedCStr};

use crate::error::BuiltinError;

pub mod parse;

pub fn execat_exec(
    dirfd: AtFd<'_>,
    pathname: &CStr,
    argv: &[&CStr],
    envp: &[ExportedCStr],
) -> Result<(), Report<BuiltinError>> {
    sys::execveat::execveat(dirfd, pathname, argv, envp, 0)
        .change_context(BuiltinError::Syscall)?;
    Ok(())
}
