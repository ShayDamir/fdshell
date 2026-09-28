//! `sendmsg %sock [--msg TEXT | --msgfd %var COUNT] [--fd %var]... [--passcred]`
//!
//! Runs in-shell: the fds sent are this process' own fd vars and the message
//! goes straight out of it — an intercept, like `read`.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::{ImportedFd, LocalFd, ShortCStr, SyscallError};

mod parse;
mod payload;

pub(crate) fn run_sendmsg(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "sendmsg", cmdline)?;
    let parsed = parse::parse(&cmdline.args)?;

    let sock = resolve_socket(&parsed.sock, cell)?;
    if parsed.passcred {
        // Opt in before sending: the kernel captures the sender's credentials
        // at send time, so a receiver's `--cred` then sees the real identity.
        sock.set_passcred().change_context(CmdError::SendmsgCred)?;
    }
    let payload = payload::build(&parsed, cell)?;
    let fds = collect_fds(&parsed.fd_vars, cell)?;
    sys::net::sendmsg(&sock, &payload, &fds).map_err(map_send_error)?;

    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_exit(0);
    Ok(true)
}

/// `%var` → dup of the var's fd (the var keeps its own); a raw number → dup
/// with CLOEXEC above the standard fds (the original is left untouched, so
/// raw sockets stay reusable by later commands).
fn resolve_socket(
    sock: &ShortCStr,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<CmdError>> {
    if let Some(name) = sock.strip_prefix(b"%") {
        let state = cell.borrow().change_context(CmdError::SendmsgFds)?;
        return state
            .fds
            .get(&name)
            .ok_or(CmdError::FdNotSet)
            .change_context(CmdError::SendmsgFds)?
            .fd
            .try_clone()
            .change_context(CmdError::SendmsgFds);
    }
    let fd = ImportedFd::try_from(sock).change_context(CmdError::SendmsgBadSocket)?;
    fd.try_dup_above(0).change_context(CmdError::SendmsgFds)
}

fn collect_fds(
    vars: &[ShortCStr],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<LocalFd>, Report<CmdError>> {
    let mut fds = Vec::new();
    for var in vars {
        let name = var.strip_prefix(b"%").ok_or(CmdError::SendmsgFds)?;
        let state = cell.borrow().change_context(CmdError::SendmsgFds)?;
        let fd = state
            .fds
            .get(&name)
            .ok_or(CmdError::FdNotSet)
            .change_context(CmdError::SendmsgFds)?
            .fd
            .try_clone()
            .change_context(CmdError::SendmsgFds)?;
        fds.push(fd);
    }
    Ok(fds)
}

/// Map a `sys::net::sendmsg` failure to a `CmdError`, keeping the chain.
fn map_send_error(e: Report<SyscallError>) -> Report<CmdError> {
    let cmd = match e.current_context() {
        SyscallError::EINVAL("sendmsg") => CmdError::SendmsgEmptyPayload,
        SyscallError::E2BIG("sendmsg") => CmdError::SendmsgTooManyFds,
        _ => CmdError::SendmsgSyscall,
    };
    Report::new(cmd).attach(e)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
