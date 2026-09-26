//! Hex parsing for the `verity` builtin's `--digest` value.

use alloc::vec::Vec;
use builtins::error::BuiltinError;
use error_stack::{Report, bail};

/// Parse a lowercase-or-uppercase hex string into bytes (case-insensitive).
pub(crate) fn parse_hex(b: &[u8]) -> Result<Vec<u8>, Report<BuiltinError>> {
    if b.is_empty() || !b.len().is_multiple_of(2) {
        bail!(BuiltinError::InvalidArgument("digest"));
    }
    let mut out = Vec::with_capacity(b.len() / 2);
    let mut i = 0;
    while i < b.len() {
        // The first char of each pair is the high nibble.
        let hi = hex_val(*b.get(i).ok_or(BuiltinError::Never)?)?;
        let lo = hex_val(*b.get(i + 1).ok_or(BuiltinError::Never)?)?;
        out.push(hi * 16 + lo);
        i += 2;
    }
    Ok(out)
}

fn hex_val(c: u8) -> Result<u8, Report<BuiltinError>> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => bail!(BuiltinError::InvalidArgument("digest")),
    }
}
