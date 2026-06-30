use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::ImportedFd;

use crate::error::{BuiltinError, ModeParseError};

pub fn wants_help(args: &[&CStr]) -> bool {
    args.iter()
        .any(|a| a.to_bytes() == b"--help" || a.to_bytes() == b"-h")
}

pub fn split(arg: &CStr) -> Result<(&[u8], Option<&CStr>), Report<BuiltinError>> {
    let bytes = arg.to_bytes_with_nul();
    if let Some(eq) = bytes.iter().position(|&c| c == b'=') {
        let key = bytes
            .get(..eq)
            .ok_or(BuiltinError::InvalidArgument("key"))?;
        let val_bytes = bytes
            .get(eq + 1..)
            .ok_or(BuiltinError::InvalidArgument("value"))?;
        let val = CStr::from_bytes_with_nul(val_bytes)
            .change_context(BuiltinError::InvalidArgument("value"))?;
        Ok((key, Some(val)))
    } else {
        let key = bytes
            .strip_suffix(b"\0")
            .ok_or(BuiltinError::InvalidArgument("arg"))?;
        Ok((key, None))
    }
}

pub fn next_val<'a>(
    args: &[&'a CStr],
    i: &mut usize,
    val: Option<&'a CStr>,
) -> Result<&'a CStr, Report<BuiltinError>> {
    match val {
        Some(v) => Ok(v),
        None => {
            let v = args.get(*i).ok_or(BuiltinError::InvalidArgument("arg"))?;
            *i += 1;
            Ok(v)
        }
    }
}

pub fn parse_mode(s: &CStr) -> Result<u64, Report<ModeParseError>> {
    let b = s.to_bytes();
    let (d, r) = if let Some(h) = b.strip_prefix(b"0x") {
        (h, 16)
    } else if let Some(o) = b.strip_prefix(b"0o") {
        (o, 8)
    } else {
        (b, 8)
    };
    let s = core::str::from_utf8(d).change_context(ModeParseError::Utf8)?;
    u64::from_str_radix(s, r).change_context(ModeParseError::ParseFailed)
}

pub fn parse_dirfd(s: &CStr) -> Result<Option<ImportedFd>, Report<BuiltinError>> {
    if s == c"AT_FDCWD" {
        Ok(None)
    } else {
        Ok(ImportedFd::try_from(s)
            .change_context(BuiltinError::Syscall)
            .map(Some)?)
    }
}
