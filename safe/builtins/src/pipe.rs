use error_stack::{Report, ResultExt};

use crate::error::BuiltinError;

pub mod parse;

pub fn pipe_exec(flags: i32, sock: &sys::LocalFd) -> Result<(), Report<BuiltinError>> {
    let (rd, wr) = sys::pipe::pipe2(flags).change_context(BuiltinError::Syscall)?;
    sock.send_fd(&rd, c"rd")
        .change_context(BuiltinError::SendFdFailed)?;
    sock.send_fd(&wr, c"wr")
        .change_context(BuiltinError::SendFdFailed)?;
    Ok(())
}
