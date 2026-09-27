use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};

use crate::error::cmd::CmdError;
use sys::ShortCStr;

/// Parsed `recvmsg` arguments.
#[cfg_attr(test, derive(Debug))]
pub(super) struct Parsed {
    /// `--cred VAR`: string var for the `PID:UID:GID` sender identity.
    pub(super) cred: Option<ShortCStr>,
    /// Socket: an fd var name (`%…`) or a raw fd number.
    pub(super) sock: ShortCStr,
    /// String var for the payload.
    pub(super) var: ShortCStr,
    /// Declared fd slots, in order (all start with `%`).
    pub(super) fd_slots: Vec<ShortCStr>,
}

/// Parses `recvmsg [--cred VAR] %sock VAR [%fdvar ...]`.
pub(super) fn parse(args: &[ShortCStr]) -> Result<Parsed, Report<CmdError>> {
    let mut cred = None;
    let mut i = 0;
    let first = args
        .first()
        .map(|f| f.as_bytes().change_context(CmdError::Never))
        .transpose()?;
    if first == Some(b"--cred".as_slice()) {
        let val = args.get(1).ok_or(CmdError::RecvmsgBadUsage)?;
        if val.starts_with(b"%") {
            bail!(CmdError::RecvmsgBadUsage);
        }
        cred = Some(val.clone());
        i = 2;
    }
    let sock = args.get(i).ok_or(CmdError::RecvmsgBadUsage)?.clone();
    let var = args.get(i + 1).ok_or(CmdError::RecvmsgBadUsage)?.clone();
    if var.starts_with(b"%") {
        bail!(CmdError::RecvmsgBadUsage);
    }
    let mut fd_slots = Vec::new();
    for slot in args.get(i + 2..).ok_or(CmdError::Never)? {
        if !slot.starts_with(b"%") {
            bail!(CmdError::RecvmsgBadUsage);
        }
        fd_slots.push(slot.clone());
    }
    Ok(Parsed {
        cred,
        sock,
        var,
        fd_slots,
    })
}
