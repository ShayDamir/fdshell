use error_stack::{Report, ResultExt};
use sys::fcntl::O_CLOEXEC;

use crate::error::BuiltinError;

pub mod parse;

pub fn pipe_exec(flags: i32) -> Result<(), Report<BuiltinError>> {
    let (rd, wr) = sys::pipe::pipe2(O_CLOEXEC | flags).change_context(BuiltinError::Syscall)?;
    sys::shellfd::send_fd(&rd, c"rd").change_context(BuiltinError::SendFdFailed)?;
    sys::shellfd::send_fd(&wr, c"wr").change_context(BuiltinError::SendFdFailed)?;
    Ok(())
}
