use alloc::vec::Vec;
use core::ffi::CStr;

use crate::error::BuiltinError;

pub struct FchmodConfig {
    pub fds: Vec<i32>,
    pub mode: u32,
}

pub fn fchmod_parse(args: &[&CStr]) -> Result<FchmodConfig, BuiltinError> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        return Err(BuiltinError::Help);
    }

    let has_flags = args.iter().any(|a| a.to_bytes().starts_with(b"-"));

    if has_flags {
        flag_mode(args)
    } else {
        positional_mode(args)
    }
}

fn flag_mode(args: &[&CStr]) -> Result<FchmodConfig, BuiltinError> {
    let mut fds: Vec<i32> = Vec::new();
    let mut mode: Option<u32> = None;
    let mut i = 0;

    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument)?;
        i += 1;
        let (key, val) = crate::argparse::split(arg)?;
        match key {
            b"--fd" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                fds.push(parse_fd(s)?);
            }
            b"--mode" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                mode = Some(crate::argparse::parse_mode(s)? as u32);
            }
            _ => return Err(BuiltinError::InvalidArgument),
        }
    }

    let mode = mode.ok_or(BuiltinError::InvalidArgument)?;
    if fds.is_empty() {
        return Err(BuiltinError::InvalidArgument);
    }
    Ok(FchmodConfig { fds, mode })
}

fn positional_mode(args: &[&CStr]) -> Result<FchmodConfig, BuiltinError> {
    if args.len() < 2 {
        return Err(BuiltinError::InvalidArgument);
    }

    let mode =
        crate::argparse::parse_mode(args.first().ok_or(BuiltinError::InvalidArgument)?)? as u32;
    let fds: Vec<i32> = args
        .iter()
        .skip(1)
        .map(|a| parse_fd(a))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(FchmodConfig { fds, mode })
}

fn parse_fd(s: &CStr) -> Result<i32, BuiltinError> {
    let b = s.to_bytes();
    let s = match core::str::from_utf8(b) {
        Ok(s) => s,
        Err(_) => return Err(BuiltinError::InvalidArgument),
    };
    let n: i32 = match s.parse() {
        Ok(n) => n,
        Err(_) => return Err(BuiltinError::InvalidArgument),
    };
    if n < 0 {
        return Err(BuiltinError::InvalidArgument);
    }
    Ok(n)
}
