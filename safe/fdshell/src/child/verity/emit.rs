//! One-line output for the `verity` builtin's query mode.

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
