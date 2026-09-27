//! `recvmsg` socket resolution and error mapping.

use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::{ImportedFd, LocalFd, Origin, ShortCStr, SyscallError};

/// Resolve the socket operand to a `LocalFd` the sys wrappers can use.
///
/// `%var` → dup of the var's fd (the var keeps its own); a raw number → dup
/// with CLOEXEC above the standard fds (the original is left untouched, so
/// raw sockets stay reusable by later commands).
pub(super) fn resolve_socket(
    sock: &ShortCStr,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<CmdError>> {
    if let Some(name) = sock.strip_prefix(b"%") {
        let state = cell
            .borrow()
            .change_context(CmdError::RecvmsgSocketSyscall)?;
        return state
            .fds
            .get(&name)
            .ok_or(CmdError::FdNotSet)
            .change_context(CmdError::RecvmsgSocketSyscall)?
            .fd
            .try_clone()
            .change_context(CmdError::RecvmsgSocketSyscall);
    }
    let fd = ImportedFd::try_from(sock).change_context(CmdError::RecvmsgBadUsage)?;
    fd.try_dup_above(0)
        .change_context(CmdError::RecvmsgSocketSyscall)
}

/// The provenance origin for every var `recvmsg` sets: the socket it came
/// from, shown by `fdexplain` as `fd {n}`.
pub(super) fn origin(sock: &ShortCStr) -> Origin {
    match sock.strip_prefix(b"%") {
        Some(name) => Origin::Read(name),
        None => Origin::Read(sock.clone()),
    }
}

/// Map a `sys::net::recvmsg` failure to a `CmdError`, keeping the chain.
pub(super) fn map_recv_error(e: Report<SyscallError>) -> Report<CmdError> {
    let cmd = match e.current_context() {
        SyscallError::E2BIG("recvmsg") => CmdError::RecvmsgTooManyFds,
        _ => CmdError::RecvmsgSyscall,
    };
    Report::new(cmd).attach(e)
}
