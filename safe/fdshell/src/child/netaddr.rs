//! Shared argument parsing for the `bind`, `listen` and `connect` builtins.
//!
//! All three take `--type stream|dgram` (default `stream`) and an ADDRESS:
//! `@name` (AF_UNIX abstract namespace, no filesystem object), `path`
//! (AF_UNIX filesystem socket, resolved against the CWD), or
//! `--addr ADDR --port N` (AF_INET v4; `--bind ADDR` is the accepted
//! historical spelling, and for `connect` the pair names the **peer**).
//! `--backlog N` (default 1) belongs to `listen` alone — `bind` and
//! `connect` never call `listen(2)`, so they reject the flag.

// mutants: -C --lib -- -E 'test(/^child::netaddr::/)'

mod address;
mod parsed;

use core::ffi::CStr;
use error_stack::{Report, bail};
use sys::ShortCStr;

use builtins::error::BuiltinError;

use address::make_address;

/// Longest address that fits the 108-byte `sun_path` (room for the
/// terminating NUL of a path or the leading NUL of an abstract name).
pub(crate) const SUN_PATH_MAX: usize = 107;

#[cfg_attr(test, derive(Debug))]
pub(crate) enum Address {
    /// AF_UNIX filesystem socket, resolved against the shell's CWD.
    UdsPath(ShortCStr),
    /// AF_UNIX abstract namespace; no filesystem object exists.
    UdsAbstract(ShortCStr),
    /// AF_INET v4 endpoint (dotted-quad, validated by `sys::net::bind_inet`).
    Inet { addr: ShortCStr, port: u16 },
}

#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub(crate) enum SocketType {
    Stream,
    Dgram,
}

#[cfg_attr(test, derive(Debug))]
pub(crate) struct NetArgs {
    pub(crate) ty: SocketType,
    pub(crate) addr: Address,
    pub(crate) backlog: Option<i32>,
}

/// `allows_backlog`: only `listen` calls `listen(2)`, so only `listen` gives
/// `--backlog` a meaning; a flag that parses and does nothing is a usage
/// error (STYLE.md §4.7).
pub(crate) fn parse_net_args(
    refs: &[&CStr],
    allows_backlog: bool,
) -> Result<NetArgs, Report<BuiltinError>> {
    if builtins::argparse::wants_help(refs) {
        bail!(BuiltinError::Help);
    }
    let mut collected = parsed::Parsed::default();
    collected.collect(refs, allows_backlog)?;
    let backlog = collected.backlog.map(address::parse_backlog).transpose()?;
    Ok(NetArgs {
        ty: collected.ty.unwrap_or(SocketType::Stream),
        addr: make_address(collected.addr, collected.bind_addr, collected.port)?,
        backlog,
    })
}

#[cfg(test)]
mod tests;
