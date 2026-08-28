use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};

use crate::error::cmd::CmdError;
use crate::intercept::signalfd_cmd::signals;
use sys::ShortCStr;

/// Parsed `signalfd` arguments: the fd var name, signal numbers, and flags.
#[cfg_attr(test, derive(Debug))]
pub(super) struct Parsed {
    pub(super) var: ShortCStr,
    pub(super) signals: Vec<i32>,
    pub(super) flags: i32,
}

/// Parses `signalfd %var <sig1> [sig2 ...] [--flags F]`.
pub(super) fn parse(args: &[ShortCStr]) -> Result<Parsed, Report<CmdError>> {
    let var = args.first().ok_or(CmdError::SignalfdNoVar)?.clone();
    if !var.starts_with(b"%") {
        bail!(CmdError::SignalfdNoVar);
    }
    let mut result_signals: Vec<i32> = Vec::new();
    let mut result_flags: i32 = 0;
    let mut i = 1;
    while i < args.len() {
        let arg = args.get(i).ok_or(CmdError::SignalfdNoVar)?;
        let bytes = arg.as_bytes().change_context(CmdError::Never)?;
        if bytes == b"--flags" {
            let val = args
                .get(i + 1)
                .ok_or(CmdError::SignalfdBadFlag { value: arg.clone() })?;
            let val_bytes = val.as_bytes().change_context(CmdError::Never)?;
            result_flags |= parse_flag(val_bytes, val)?;
            i += 2;
        } else if bytes.starts_with(b"-") {
            bail!(CmdError::SignalfdBadFlag { value: arg.clone() });
        } else {
            result_signals.push(signals::parse_signal(bytes, arg)?);
            i += 1;
        }
    }
    if result_signals.is_empty() {
        bail!(CmdError::SignalfdNoVar);
    }
    Ok(Parsed {
        var,
        signals: result_signals,
        flags: result_flags,
    })
}

fn parse_flag(bytes: &[u8], val: &ShortCStr) -> Result<i32, Report<CmdError>> {
    if let Some(hex) = bytes.strip_prefix(b"0x") {
        let h = core::str::from_utf8(hex).change_context(CmdError::Never)?;
        return i32::from_str_radix(h, 16)
            .map_err(|_| Report::new(CmdError::SignalfdBadFlag { value: val.clone() }));
    }
    match bytes {
        b"SFD_NONBLOCK" => Ok(sys::signalfd::SFD_NONBLOCK),
        _ => Err(Report::new(CmdError::SignalfdBadFlag {
            value: val.clone(),
        })),
    }
}
