//! `bind [--type stream|dgram] ADDRESS` — create a socket and bind it; the
//! new socket is captured into an fd var via `%>%var` (bounded form
//! `%>%array[N]`).
//!
//! ADDRESS: `@name` is an AF_UNIX abstract-namespace name (no filesystem
//! object); `path` is an AF_UNIX filesystem socket resolved against the
//! CWD (the script owns the path and must unlink it after use);
//! `--bind ADDR --port N` is AF_INET v4.

use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};
use sys::LocalFd;

use super::Ctx;
use super::netaddr::{self, Address, SocketType};

pub(super) fn handle_bind(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = netaddr::parse_net_args(ctx.refs)?;
    let sock = shell_sock(ctx)?;
    let fd = create_socket(cfg.ty, &cfg.addr)?;
    bind_socket(&fd, &cfg.addr)?;
    sock.send_fd(&fd, c"bind")
        .change_context(BuiltinError::SendFdFailed)?;
    Ok(0)
}

/// The capture socket the parent waits on; the child must report through it.
pub(super) fn shell_sock<'a>(ctx: &'a Ctx<'_>) -> Result<&'a LocalFd, Report<BuiltinError>> {
    Ok(ctx
        .state
        .shell_sock
        .as_ref()
        .ok_or(BuiltinError::SendFdFailed)?)
}

/// Shared with `listen`: open the socket matching the address family and
/// type, and bind it to the parsed address.
pub(super) fn create_socket(
    ty: SocketType,
    addr: &Address,
) -> Result<LocalFd, Report<BuiltinError>> {
    let domain = match addr {
        Address::UdsPath(_) | Address::UdsAbstract(_) => sys::net::AF_UNIX,
        Address::Inet { .. } => sys::net::AF_INET,
    };
    let kind = match ty {
        SocketType::Stream => sys::net::SOCK_STREAM,
        SocketType::Dgram => sys::net::SOCK_DGRAM,
    };
    sys::net::socket(domain, kind).change_context(BuiltinError::Syscall)
}

pub(super) fn bind_socket(fd: &LocalFd, addr: &Address) -> Result<(), Report<BuiltinError>> {
    match addr {
        Address::UdsPath(p) => {
            sys::net::bind_uds_path(fd, p.export()).change_context(BuiltinError::Syscall)
        }
        Address::UdsAbstract(n) => {
            sys::net::bind_uds_abstract(fd, n.export()).change_context(BuiltinError::Syscall)
        }
        Address::Inet { addr, port } => {
            sys::net::bind_inet(fd, addr.export(), *port).change_context(BuiltinError::Syscall)
        }
    }
}

#[cfg(test)]
mod tests;
