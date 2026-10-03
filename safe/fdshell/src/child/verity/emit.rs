//! Output for the `verity` builtin: the query-mode line and the `--dump`
//! hex rows.

use alloc::string::String;
use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};
use sys::fsverity::FsverityDigest;

/// Hash-algorithm name for a measured uapi algorithm code.
pub(crate) fn algo_name(code: u16) -> &'static str {
    match code {
        1 => "sha256",
        2 => "sha512",
        _ => "unknown",
    }
}

/// Lowercase hex encoding of `b` (two chars per byte).
pub(crate) fn to_hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push_str(&alloc::format!("{x:02x}"));
    }
    s
}

/// `hexdump`-style rows over `data`, starting at the absolute byte `offset`:
/// 16 bytes per row, an 8-hex-digit row offset, a mid-row gap after the 8th
/// byte, and a partial last row padded with `__`.
pub(crate) fn write_hex_rows(
    offset: u64,
    data: &[u8],
) -> Result<sys::ShortCStr, Report<BuiltinError>> {
    let mut out = sys::ShortCStr::new();
    let mut off = offset;
    for row in data.chunks(16) {
        let line = hex_row(off, row).change_context(BuiltinError::Io)?;
        sys::write!(&mut out, "{line}").change_context(BuiltinError::Io)?;
        off += row.len() as u64;
    }
    Ok(out)
}

/// One hex row: `<offset:08x>  <8 bytes>  <8 bytes>\n`, `__` padding.
fn hex_row(offset: u64, data: &[u8]) -> Result<sys::ShortCStr, Report<BuiltinError>> {
    let mut s = sys::ShortCStr::new();
    sys::write!(&mut s, "{offset:08x}  ").change_context(BuiltinError::Io)?;
    for i in 0..16 {
        if i == 8 {
            sys::write!(&mut s, " ").change_context(BuiltinError::Io)?;
        }
        match data.get(i) {
            Some(x) => sys::write!(&mut s, "{x:02x}"),
            None => sys::write!(&mut s, "__"),
        }
        .change_context(BuiltinError::Io)?;
        if i < 15 {
            sys::write!(&mut s, " ").change_context(BuiltinError::Io)?;
        }
    }
    sys::write!(&mut s, "\n").change_context(BuiltinError::Io)?;
    Ok(s)
}

/// `enabled=yes algo=.. digest=..` or `enabled=no`, plus a newline.
pub(crate) fn line(d: Option<&FsverityDigest>) -> Result<sys::ShortCStr, Report<BuiltinError>> {
    let out = match d {
        None => "enabled=no\n",
        Some(digest) => {
            return sys::format!(
                "enabled=yes algo={} digest={}\n",
                algo_name(digest.algorithm),
                to_hex(&digest.digest)
            )
            .change_context(BuiltinError::Io);
        }
    };
    sys::format!("{out}").change_context(BuiltinError::Io)
}
