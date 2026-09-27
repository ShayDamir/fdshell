use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};

use crate::error::cmd::CmdError;
use sys::ShortCStr;

/// Parsed `sendmsg` arguments.
#[cfg_attr(test, derive(Debug))]
pub(super) struct Parsed {
    /// Socket: an fd var name (`%…`) or a raw fd number.
    pub(super) sock: ShortCStr,
    /// `--msg TEXT`: inline payload bytes.
    pub(super) msg: Option<Vec<u8>>,
    /// `--msgfd %var COUNT`: read the payload from an fd var.
    pub(super) msgfd: Option<(ShortCStr, usize)>,
    /// `--fd %var` occurrences in order (duplicates allowed).
    pub(super) fd_vars: Vec<ShortCStr>,
    /// `--passcred`: enable `SO_PASSCRED` so the message carries real creds.
    pub(super) passcred: bool,
}

/// Parses `sendmsg %sock [--msg TEXT | --msgfd %var COUNT] [--fd %var]...
/// [--passcred]`.
pub(super) fn parse(args: &[ShortCStr]) -> Result<Parsed, Report<CmdError>> {
    let sock = args.first().ok_or(CmdError::SendmsgBadSocket)?.clone();
    let mut msg = None;
    let mut msgfd = None;
    let mut fd_vars = Vec::new();
    let mut passcred = false;
    let mut i = 1;
    while i < args.len() {
        let arg = args.get(i).ok_or(CmdError::SendmsgUsage)?;
        let bytes = arg.as_bytes().change_context(CmdError::Never)?;
        if bytes == b"--msg" {
            let val = args.get(i + 1).ok_or(CmdError::SendmsgUsage)?;
            if msg.is_some() || msgfd.is_some() {
                bail!(CmdError::SendmsgPayloadConflict);
            }
            msg = Some(val.as_bytes().change_context(CmdError::Never)?.to_vec());
            i += 2;
        } else if bytes == b"--msgfd" {
            let var = args.get(i + 1).ok_or(CmdError::SendmsgUsage)?;
            let count = args.get(i + 2).ok_or(CmdError::SendmsgUsage)?;
            if msg.is_some() || msgfd.is_some() {
                bail!(CmdError::SendmsgPayloadConflict);
            }
            msgfd = Some((var.clone(), parse_count(count)?));
            i += 3;
        } else if bytes == b"--fd" {
            let var = args.get(i + 1).ok_or(CmdError::SendmsgUsage)?;
            if !var.starts_with(b"%") {
                bail!(CmdError::SendmsgFds);
            }
            fd_vars.push(var.clone());
            i += 2;
        } else if bytes == b"--passcred" {
            passcred = true;
            i += 1;
        } else {
            bail!(CmdError::SendmsgUsage);
        }
    }
    Ok(Parsed {
        sock,
        msg,
        msgfd,
        fd_vars,
        passcred,
    })
}

fn parse_count(val: &ShortCStr) -> Result<usize, Report<CmdError>> {
    let s = core::str::from_utf8(val.as_bytes().change_context(CmdError::Never)?)
        .change_context(CmdError::SendmsgUsage)?;
    s.parse::<usize>().change_context(CmdError::SendmsgUsage)
}
