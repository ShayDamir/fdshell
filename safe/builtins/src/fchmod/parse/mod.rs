use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};

use crate::error::{BuiltinError, Suggestion};
use sys::ImportedFd;

pub struct FchmodConfig {
    pub fds: Vec<ImportedFd>,
    pub mode: u32,
}

pub fn fchmod_parse(args: &[&CStr]) -> Result<FchmodConfig, Report<BuiltinError>> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        bail!(BuiltinError::Help);
    }

    let has_flags = args.iter().any(|a| a.to_bytes().starts_with(b"-"));

    if has_flags {
        flag_mode(args)
    } else {
        positional_mode(args)
    }
}

fn flag_mode(args: &[&CStr]) -> Result<FchmodConfig, Report<BuiltinError>> {
    let mut fds: Vec<ImportedFd> = Vec::new();
    let mut mode: Option<u32> = None;
    let mut i = 0;

    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument("arg"))?;
        i += 1;
        let (key, val) = crate::argparse::split(arg)?;
        match key {
            b"--fd" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                let fd = ImportedFd::from_bytes(s.to_bytes())
                    .change_context(BuiltinError::InvalidArgument("fd"))?;
                fds.push(fd);
            }
            b"--mode" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                let m = crate::argparse::parse_mode(s)
                    .change_context(BuiltinError::InvalidArgument("mode"))
                    .attach_opaque(Suggestion(
                        "Use octal without prefix (e.g. 755) or hex with 0x prefix (e.g. 0x1ff)",
                    ))? as u32;
                mode = Some(m);
            }
            _ => bail!(BuiltinError::InvalidArgument("flag")),
        }
    }

    let mode = mode.ok_or(BuiltinError::InvalidArgument("mode"))?;
    if fds.is_empty() {
        bail!(BuiltinError::InvalidArgument("fd"));
    }
    Ok(FchmodConfig { fds, mode })
}

fn positional_mode(args: &[&CStr]) -> Result<FchmodConfig, Report<BuiltinError>> {
    if args.is_empty() {
        bail!(BuiltinError::MissingArgument("mode"));
    }
    if args.len() == 1 {
        bail!(BuiltinError::MissingArgument("fd"));
    }

    let mode =
        crate::argparse::parse_mode(args.first().ok_or(BuiltinError::InvalidArgument("mode"))?)
            .change_context(BuiltinError::InvalidArgument("mode"))
            .attach_opaque(Suggestion(
                "Use octal without prefix (e.g. 755) or hex with 0x prefix (e.g. 0x1ff)",
            ))? as u32;

    let mut fds = Vec::new();
    for a in args.iter().skip(1) {
        let fd = ImportedFd::from_bytes(a.to_bytes())
            .change_context(BuiltinError::InvalidArgument("fd"))?;
        fds.push(fd);
    }

    Ok(FchmodConfig { fds, mode })
}
