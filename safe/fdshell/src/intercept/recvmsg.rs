//! `recvmsg [--cred VAR] %sock VAR [%fdvar ...]`
//!
//! Runs in-shell: it blocks on the socket (the fd stays in this process'
//! table) and commits payload, fd vars and sender credentials in one state
//! mutation — an intercept, like `read`.

use alloc::vec;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::{FdVar, ShellState};
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ScriptText, ShortCStr, Trace};

mod io;
mod parse;

/// One payload receive is capped here; larger payloads span several
/// `recvmsg` calls, their fds riding the first one.
const MAX_PAYLOAD: usize = 64 * 1024;

pub(crate) fn run_recvmsg(
    line: &[u8],
    cmdline: &CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "recvmsg", cmdline)?;
    let parsed = parse::parse(&cmdline.args)?;

    let sock = io::resolve_socket(&parsed.sock, cell)?;
    if parsed.cred.is_some() {
        sys::net::set_passcred(&sock).change_context(CmdError::RecvmsgCred)?;
    }

    let mut buf = vec![0u8; MAX_PAYLOAD];
    let msg = sys::net::recvmsg(
        &sock,
        &mut buf,
        parsed.fd_slots.len(),
        parsed.cred.is_some(),
    )
    .map_err(io::map_recv_error)?;
    let origin = io::origin(&parsed.sock);

    if msg.eof {
        // No data, no fds: the peer closed the connection. The var is set
        // empty and the command fails, so `wait` arms and `if` chains see it.
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.set_var(
            parsed.var,
            ImportedStr::new(ShortCStr::new(), Trace::at(text.start, origin)),
        );
        state.set_last_exit(1);
        return Ok(true);
    }

    if msg.fds.len() != parsed.fd_slots.len() {
        // Dropping `msg` closes every received fd; no vars are set.
        return Err(CmdError::RecvmsgFdCountMismatch.into());
    }

    let payload = ShortCStr::from_vec(buf.get(..msg.payload_len).ok_or(CmdError::Never)?.to_vec())
        .change_context(CmdError::RecvmsgPayloadNul)?;

    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_var(
        parsed.var,
        ImportedStr::new(payload, Trace::at(text.start, origin.clone())),
    );
    if let (Some(cred_var), Some((pid, uid, gid))) = (parsed.cred, msg.cred) {
        let cred = sys::format!("{pid}:{uid}:{gid}").change_context(CmdError::RecvmsgCred)?;
        state.set_var(
            cred_var,
            ImportedStr::new(cred, Trace::at(text.start, origin.clone())),
        );
    }
    for (slot, fd) in parsed.fd_slots.iter().zip(msg.fds) {
        let name = slot.strip_prefix(b"%").ok_or(CmdError::RecvmsgBadUsage)?;
        state.set_fd_var(
            name,
            FdVar {
                fd,
                trace: Trace::at(text.start, origin.clone()),
            },
        );
    }
    state.set_last_exit(0);
    Ok(true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
