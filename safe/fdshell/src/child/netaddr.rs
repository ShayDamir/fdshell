//! Shared argument parsing for the `bind` and `listen` builtins.
//!
//! Both take `--type stream|dgram` (default `stream`) and an ADDRESS:
//! `@name` (AF_UNIX abstract namespace, no filesystem object), `path`
//! (AF_UNIX filesystem socket, resolved against the CWD), or
//! `--bind ADDR --port N` (AF_INET v4). `listen` additionally takes
//! `--backlog N` (default 1).

mod address;

use core::ffi::CStr;
use error_stack::{Report, bail, ensure};
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

pub(crate) fn parse_net_args(refs: &[&CStr]) -> Result<NetArgs, Report<BuiltinError>> {
    if builtins::argparse::wants_help(refs) {
        bail!(BuiltinError::Help);
    }
    let mut ty: Option<SocketType> = None;
    let mut bind_addr: Option<&CStr> = None;
    let mut port: Option<&CStr> = None;
    let mut backlog: Option<&CStr> = None;
    let mut addr: Option<&CStr> = None;
    let mut i = 0;
    while i < refs.len() {
        let arg = refs.get(i).ok_or(BuiltinError::Never)?;
        i += 1;
        let (key, val) = builtins::argparse::split(arg)?;
        match key {
            b"--type" => {
                ensure!(ty.is_none(), BuiltinError::InvalidArgument("type"));
                let v = builtins::argparse::next_val(refs, &mut i, val)?;
                ty = Some(address::parse_type(v)?);
            }
            b"--bind" => {
                ensure!(bind_addr.is_none(), BuiltinError::InvalidArgument("bind"));
                bind_addr = Some(builtins::argparse::next_val(refs, &mut i, val)?);
            }
            b"--port" => {
                ensure!(port.is_none(), BuiltinError::InvalidArgument("port"));
                port = Some(builtins::argparse::next_val(refs, &mut i, val)?);
            }
            b"--backlog" => {
                ensure!(backlog.is_none(), BuiltinError::InvalidArgument("backlog"));
                backlog = Some(builtins::argparse::next_val(refs, &mut i, val)?);
            }
            a if a.starts_with(b"-") => bail!(BuiltinError::InvalidArgument("flag")),
            _ => {
                ensure!(addr.is_none(), BuiltinError::InvalidArgument("address"));
                addr = Some(arg);
            }
        }
    }
    let backlog = backlog.map(address::parse_backlog).transpose()?;
    Ok(NetArgs {
        ty: ty.unwrap_or(SocketType::Stream),
        addr: make_address(addr, bind_addr, port)?,
        backlog,
    })
}

#[cfg(test)]
mod tests;
