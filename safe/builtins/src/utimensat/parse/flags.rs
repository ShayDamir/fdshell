use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};

use crate::error::FlagParseError;

/// Parses an `utimensat` flag value: the name `AT_SYMLINK_NOFOLLOW` or a
/// `0x`-prefixed hex number. `AT_SYMLINK_NOFOLLOW` is the only legal named
/// flag, so this is a direct match, not a `|`-split fold (a single-flag fold is
/// dead generality and a needless source of equivalent mutants).
pub(crate) fn parse_utime_flags(s: &CStr) -> Result<i32, Report<FlagParseError>> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let bytes = b.get(2..).ok_or(FlagParseError::Unknown)?;
        let h = core::str::from_utf8(bytes).change_context(FlagParseError::Utf8)?;
        return i32::from_str_radix(h, 16).change_context(FlagParseError::HexParse);
    }
    if b == b"AT_SYMLINK_NOFOLLOW" {
        return Ok(sys::fileat::AT_SYMLINK_NOFOLLOW);
    }
    bail!(FlagParseError::Unknown)
}
