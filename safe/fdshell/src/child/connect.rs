//! `connect [--type stream|dgram] ADDRESS` — create a socket and connect it
//! to a peer address; the connected socket is captured into an fd var via
//! `%>%var` (bounded form `%>%array[N]`, tagged form `%connect>%array[N]`).
//!
//! ADDRESS: `@name` is an AF_UNIX abstract-namespace name (the peer must be
//! bound and, for stream sockets, listening); `path` is an AF_UNIX
//! filesystem socket that must already exist (`connect` never creates one);
//! `--bind ADDR --port N` is AF_INET v4. For `connect` that pair names the
//! **peer** endpoint, not a local bind.

use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};
use sys::LocalFd;

use super::Ctx;
use super::bind::{create_socket, shell_sock};
use super::netaddr::{self, Address};

pub(super) fn handle_connect(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = netaddr::parse_net_args(ctx.refs)?;
    let sock = shell_sock(ctx)?;
    let fd = create_socket(cfg.ty, &cfg.addr)?;
    connect_socket(&fd, &cfg.addr)?;
    sock.send_fd(&fd, c"connect")
        .change_context(BuiltinError::SendFdFailed)?;
    Ok(0)
}

/// Connect `fd` to the parsed `addr` (the peer endpoint).
pub(super) fn connect_socket(fd: &LocalFd, addr: &Address) -> Result<(), Report<BuiltinError>> {
    match addr {
        Address::UdsPath(p) => {
            sys::net::connect_uds_path(fd, p.export()).change_context(BuiltinError::Syscall)
        }
        Address::UdsAbstract(n) => {
            sys::net::connect_uds_abstract(fd, n.export()).change_context(BuiltinError::Syscall)
        }
        Address::Inet { addr, port } => {
            sys::net::connect_inet(fd, addr.export(), *port).change_context(BuiltinError::Syscall)
        }
    }
}

#[cfg(test)]
mod tests;
