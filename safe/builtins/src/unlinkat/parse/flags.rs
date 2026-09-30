use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};

use crate::error::FlagParseError;

/// Parses an `unlinkat` flag value: the name `AT_REMOVEDIR` or a `0x`-prefixed
/// hex number. `AT_REMOVEDIR` is the only legal flag, so this is a direct match,
/// not a `|`-split fold (a single-flag fold is dead generality and a needless
/// source of equivalent mutants).
pub(crate) fn parse_unlink_flags(s: &CStr) -> Result<i32, Report<FlagParseError>> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let bytes = b.get(2..).ok_or(FlagParseError::Unknown)?;
        let h = core::str::from_utf8(bytes).change_context(FlagParseError::Utf8)?;
        return i32::from_str_radix(h, 16).change_context(FlagParseError::HexParse);
    }
    if b == b"AT_REMOVEDIR" {
        return Ok(sys::fileat::AT_REMOVEDIR);
    }
    bail!(FlagParseError::Unknown)
}
