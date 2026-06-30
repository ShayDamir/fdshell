use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};

use crate::error::{BuiltinError, FdParseError, Suggestion};

pub struct FchmodConfig {
    pub fds: Vec<i32>,
    pub mode: u32,
}

pub fn fchmod_parse(args: &[&CStr]) -> Result<FchmodConfig, Report<BuiltinError>> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        return Err(Report::new(BuiltinError::Help));
    }

    let has_flags = args.iter().any(|a| a.to_bytes().starts_with(b"-"));

    if has_flags {
        flag_mode(args)
    } else {
        positional_mode(args)
    }
}

fn flag_mode(args: &[&CStr]) -> Result<FchmodConfig, Report<BuiltinError>> {
    let mut fds: Vec<i32> = Vec::new();
    let mut mode: Option<u32> = None;
    let mut i = 0;

    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument("arg"))?;
        i += 1;
        let (key, val) = crate::argparse::split(arg)?;
        match key {
            b"--fd" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                let fd = parse_fd(s)
                    .change_context(BuiltinError::InvalidArgument("fd"))
                    .attach_opaque(Suggestion("Use a valid open file descriptor number"))?;
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
            _ => return Err(Report::new(BuiltinError::InvalidArgument("flag"))),
        }
    }

    let mode = mode.ok_or(BuiltinError::InvalidArgument("mode"))?;
    if fds.is_empty() {
        return Err(Report::new(BuiltinError::InvalidArgument("mode")));
    }
    Ok(FchmodConfig { fds, mode })
}

fn positional_mode(args: &[&CStr]) -> Result<FchmodConfig, Report<BuiltinError>> {
    if args.len() < 2 {
        return Err(Report::new(BuiltinError::InvalidArgument("mode")));
    }

    let mode =
        crate::argparse::parse_mode(args.first().ok_or(BuiltinError::InvalidArgument("mode"))?)
            .change_context(BuiltinError::InvalidArgument("mode"))
            .attach_opaque(Suggestion(
                "Use octal without prefix (e.g. 755) or hex with 0x prefix (e.g. 0x1ff)",
            ))? as u32;

    let mut fds = Vec::new();
    for a in args.iter().skip(1) {
        let fd = parse_fd(a)
            .change_context(BuiltinError::InvalidArgument("fd"))
            .attach_opaque(Suggestion("Use a valid open file descriptor number"))?;
        fds.push(fd);
    }

    Ok(FchmodConfig { fds, mode })
}

fn parse_fd(s: &CStr) -> Result<i32, FdParseError> {
    let b = s.to_bytes();
    let s = match core::str::from_utf8(b) {
        Ok(s) => s,
        Err(_) => return Err(FdParseError::Invalid),
    };
    let n: i32 = match s.parse() {
        Ok(n) => n,
        Err(_) => return Err(FdParseError::Invalid),
    };
    if n < 0 {
        return Err(FdParseError::Negative);
    }
    Ok(n)
}
