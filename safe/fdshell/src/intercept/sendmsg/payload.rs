//! `sendmsg` payload construction: inline `--msg` or `--msgfd %var COUNT`.

use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use super::parse::Parsed;
use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

/// Build the payload bytes. NUL bytes in a `--msgfd` payload are an error:
/// the payload travels as a string var on the receive side.
pub(super) fn build(
    parsed: &Parsed,
    cell: &ForkCell<ShellState>,
) -> Result<Vec<u8>, Report<CmdError>> {
    let Some((var, count)) = &parsed.msgfd else {
        return Ok(parsed.msg.clone().unwrap_or_default());
    };
    let name = var.strip_prefix(b"%").ok_or(CmdError::SendmsgFds)?;
    let fd = {
        let state = cell.borrow().change_context(CmdError::SendmsgFds)?;
        state
            .fds
            .get(&name)
            .ok_or(CmdError::FdNotSet)
            .change_context(CmdError::SendmsgFds)?
            .fd
            .try_clone()
            .change_context(CmdError::SendmsgFds)?
    };
    let mut buf = vec![0u8; *count];
    let n = fd
        .read_all(&mut buf)
        .change_context(CmdError::SendmsgSyscall)?;
    buf.truncate(n);
    if buf.contains(&0) {
        return Err(CmdError::SendmsgBadPayload.into());
    }
    Ok(buf)
}
