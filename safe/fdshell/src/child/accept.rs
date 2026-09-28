//! `accept %fd` — blocking accept on a listening fd var; the accepted
//! connection is captured into a fd var via `%>%var` (bounded form
//! `%>%array[N]`).
//!
//! Blocks until a connection arrives. With no capture, the connection is
//! accepted and closed immediately (the `pipe` precedent). For the
//! non-blocking form, background it (`builtin accept %l %>%c &>&x`) or run
//! it under a `wait` arm (`readable %l %>%conns[N])`).

use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt, bail};

use super::Ctx;
use super::bind::shell_sock;
use super::fdops::args::{no_extra, var_arg};

pub(super) fn handle_accept(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    if builtins::argparse::wants_help(ctx.refs) {
        bail!(BuiltinError::Help);
    }
    let var = var_arg(ctx.args)?;
    no_extra(ctx.refs.len(), 1)?;
    let found = ctx.state.fds.get(&var).ok_or(BuiltinError::FdVarNotFound)?;
    let conn = sys::net::accept(&found.fd).change_context(BuiltinError::Syscall)?;
    let sock = shell_sock(ctx)?;
    sock.send_fd(&conn, c"accept")
        .change_context(BuiltinError::SendFdFailed)?;
    Ok(0)
}

#[cfg(test)]
mod tests;
