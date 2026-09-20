use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::{ExportedCStr, LocalFd};

use crate::error::BuiltinError;

pub mod parse;

pub fn execfd_exec(
    fd: &LocalFd,
    argv: &[&CStr],
    envp: &[ExportedCStr],
) -> Result<(), Report<BuiltinError>> {
    let script_fd = fd.export().change_context(BuiltinError::Syscall)?;
    sys::execveat::execveat(
        script_fd.at(),
        c"",
        argv,
        envp,
        sys::execveat::AT_EMPTY_PATH,
    )
    .change_context(BuiltinError::Syscall)?;
    Ok(())
}
