//! `listen [--type stream|dgram] [--backlog N] ADDRESS` — create, bind, and
//! listen on a socket in one step; the listening socket is captured into an
//! fd var via `%>%var` (bounded form `%>%array[N]`).
//!
//! Address forms and defaults are the same as `bind` (see `netaddr`); the
//! backlog defaults to 1.

use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};

use super::Ctx;
use super::bind::{bind_socket, create_socket, shell_sock};
use super::netaddr;

pub(super) fn handle_listen(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = netaddr::parse_net_args(ctx.refs)?;
    let sock = shell_sock(ctx)?;
    let fd = create_socket(cfg.ty, &cfg.addr)?;
    bind_socket(&fd, &cfg.addr)?;
    sys::net::listen(&fd, cfg.backlog.unwrap_or(1)).change_context(BuiltinError::Syscall)?;
    sock.send_fd(&fd, c"listen")
        .change_context(BuiltinError::SendFdFailed)?;
    Ok(0)
}

#[cfg(test)]
mod tests;
