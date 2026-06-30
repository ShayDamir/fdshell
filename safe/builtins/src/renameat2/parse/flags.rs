use core::ffi::CStr;
use error_stack::{Report, ResultExt};

use crate::error::FlagParseError;

pub(crate) fn parse_rename_flags(s: &CStr) -> Result<u32, Report<FlagParseError>> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let bytes = b.get(2..).ok_or(FlagParseError::Unknown)?;
        let h = core::str::from_utf8(bytes).change_context(FlagParseError::Utf8)?;
        u32::from_str_radix(h, 16).change_context(FlagParseError::HexParse)
    } else {
        b.split(|&c| c == b'|')
            .try_fold(0, |acc, name| {
                let v = match name {
                    b"RENAME_NOREPLACE" => sys::renameat2::RENAME_NOREPLACE,
                    b"RENAME_EXCHANGE" => sys::renameat2::RENAME_EXCHANGE,
                    b"RENAME_WHITEOUT" => sys::renameat2::RENAME_WHITEOUT,
                    _ => return Err(FlagParseError::Unknown),
                };
                Ok(acc | v)
            })
            .change_context(FlagParseError::Unknown)
    }
}
