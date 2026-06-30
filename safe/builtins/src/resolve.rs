use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::openat2::{
    RESOLVE_BENEATH, RESOLVE_CACHED, RESOLVE_IN_ROOT, RESOLVE_NO_MAGICLINKS, RESOLVE_NO_SYMLINKS,
    RESOLVE_NO_XDEV,
};

use crate::error::FlagParseError;

pub fn parse_resolve_flags(s: &CStr) -> Result<u64, Report<FlagParseError>> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let bytes = b.get(2..).ok_or(FlagParseError::Unknown)?;
        let h = core::str::from_utf8(bytes).change_context(FlagParseError::Utf8)?;
        u64::from_str_radix(h, 16).change_context(FlagParseError::HexParse)
    } else {
        b.split(|&c| c == b'|')
            .try_fold(0, |acc, name| {
                let v = match name {
                    b"RESOLVE_NO_SYMLINKS" => RESOLVE_NO_SYMLINKS,
                    b"RESOLVE_NO_MAGICLINKS" => RESOLVE_NO_MAGICLINKS,
                    b"RESOLVE_NO_XDEV" => RESOLVE_NO_XDEV,
                    b"RESOLVE_BENEATH" => RESOLVE_BENEATH,
                    b"RESOLVE_IN_ROOT" => RESOLVE_IN_ROOT,
                    b"RESOLVE_CACHED" => RESOLVE_CACHED,
                    _ => return Err(FlagParseError::Unknown),
                };
                Ok(acc | v)
            })
            .change_context(FlagParseError::Unknown)
    }
}
