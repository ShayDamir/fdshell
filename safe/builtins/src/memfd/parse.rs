//! `memfd` argument parsing: `--name NAME`, `--size BYTES`, `--seal FLAG`.

use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};

use crate::argparse;
use crate::error::BuiltinError;

use sys::memfd;

#[cfg_attr(test, derive(Debug))]
pub struct MemfdConfig<'a> {
    pub name: Option<&'a CStr>,
    pub size: Option<u64>,
    pub seals: u32,
}

pub fn memfd_parse<'a>(args: &[&'a CStr]) -> Result<MemfdConfig<'a>, Report<BuiltinError>> {
    if argparse::wants_help(args) {
        bail!(BuiltinError::Help);
    }
    // `args` are the command's arguments (the name is dispatched separately), so
    // a call with none is valid: an anonymous memfd. Index advances by one for
    // the token here and again inside `next_val` for a flag's value.
    let mut name: Option<&CStr> = None;
    let mut size: Option<u64> = None;
    let mut seals: u32 = 0;
    let mut i = 0;
    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument("arg"))?;
        i += 1;
        let (key, val) = argparse::split(arg)?;
        match key {
            b"--name" => name = Some(parse_name(argparse::next_val(args, &mut i, val)?)?),
            b"--size" => size = Some(parse_size(argparse::next_val(args, &mut i, val)?)?),
            b"--seal" => seals |= parse_seal(argparse::next_val(args, &mut i, val)?)?,
            other if other.starts_with(b"-") => bail!(BuiltinError::InvalidArgument("flag")),
            _ => bail!(BuiltinError::InvalidArgument("arg")),
        }
    }
    Ok(MemfdConfig { name, size, seals })
}

/// A memfd name must be non-empty, ≤ 14 bytes, and contain no `/`; the NUL
/// terminator is not part of `to_bytes()`, so NUL is impossible here. `memfd`
/// itself rejects these with ENAMETOOLONG / EINVAL, but a parser error is a
/// cleaner message and fails before creating the file.
fn parse_name(arg: &CStr) -> Result<&CStr, Report<BuiltinError>> {
    let bytes = arg.to_bytes();
    if bytes.is_empty() || bytes.contains(&b'/') || bytes.len() > 14 {
        bail!(BuiltinError::InvalidArgument("name"));
    }
    Ok(arg)
}

fn parse_size(arg: &CStr) -> Result<u64, Report<BuiltinError>> {
    let s = core::str::from_utf8(arg.to_bytes())
        .change_context(BuiltinError::InvalidArgument("size"))?;
    s.parse::<u64>()
        .change_context(BuiltinError::InvalidArgument("size"))
}

fn parse_seal(arg: &CStr) -> Result<u32, Report<BuiltinError>> {
    match arg.to_bytes() {
        b"SHRINK" => Ok(memfd::F_SEAL_SHRINK as u32),
        b"GROW" => Ok(memfd::F_SEAL_GROW as u32),
        b"WRITE" => Ok(memfd::F_SEAL_WRITE as u32),
        b"SEAL" => Ok(memfd::F_SEAL_SEAL as u32),
        b"FUTURE_WRITE" => Ok(memfd::F_SEAL_FUTURE_WRITE as u32),
        b"EXEC" => Ok(memfd::F_SEAL_EXEC as u32),
        _ => bail!(BuiltinError::InvalidArgument("seal")),
    }
}
