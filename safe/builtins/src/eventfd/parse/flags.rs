use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::eventfd::{EFD_NONBLOCK, EFD_SEMAPHORE};

use crate::error::FlagParseError;

pub(crate) fn parse_eventfd_flag(s: &CStr) -> Result<i32, Report<FlagParseError>> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let bytes = b.get(2..).ok_or(FlagParseError::Unknown)?;
        let h = core::str::from_utf8(bytes).change_context(FlagParseError::Utf8)?;
        i32::from_str_radix(h, 16).change_context(FlagParseError::HexParse)
    } else {
        match b {
            b"EFD_NONBLOCK" => Ok(EFD_NONBLOCK),
            b"EFD_SEMAPHORE" => Ok(EFD_SEMAPHORE),
            _ => Err(FlagParseError::Unknown),
        }
        .change_context(FlagParseError::Unknown)
    }
}
