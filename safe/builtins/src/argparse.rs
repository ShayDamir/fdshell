use core::ffi::CStr;
use sys::openat2::{
    RESOLVE_BENEATH, RESOLVE_CACHED, RESOLVE_IN_ROOT, RESOLVE_NO_MAGICLINKS, RESOLVE_NO_SYMLINKS,
    RESOLVE_NO_XDEV,
};
use sys::DupFd;

/// Checks if any argument is `--help` or `-h`.
pub fn wants_help(args: &[&CStr]) -> bool {
    args.iter()
        .any(|a| a.to_bytes() == b"--help" || a.to_bytes() == b"-h")
}

/// Splits `--key=value` into `(b"--key", Some(cstr_value))`.
/// Without `=`, returns `(bytes_without_nul, None)`.
pub fn split(arg: &CStr) -> Result<(&[u8], Option<&CStr>), i32> {
    let bytes = arg.to_bytes_with_nul();
    if let Some(eq) = bytes.iter().position(|&c| c == b'=') {
        let key = bytes.get(..eq).ok_or(22)?;
        let val = CStr::from_bytes_with_nul(bytes.get(eq + 1..).ok_or(22)?).map_err(|_| 22)?;
        Ok((key, Some(val)))
    } else {
        let key = bytes.strip_suffix(b"\0").ok_or(22)?;
        Ok((key, None))
    }
}

/// Returns `val` if `Some`, otherwise consumes the next positional argument.
pub fn next_val<'a>(
    args: &[&'a CStr],
    i: &mut usize,
    val: Option<&'a CStr>,
) -> Result<&'a CStr, i32> {
    match val {
        Some(v) => Ok(v),
        None => {
            let v = args.get(*i).ok_or(22)?;
            *i += 1;
            Ok(v)
        }
    }
}

/// Parses a mode string: octal (default), hex (`0x`), or octal with prefix (`0o`).
pub fn parse_mode(s: &CStr) -> Result<u64, i32> {
    let b = s.to_bytes();
    let (d, r) = if let Some(h) = b.strip_prefix(b"0x") {
        (h, 16)
    } else if let Some(o) = b.strip_prefix(b"0o") {
        (o, 8)
    } else {
        (b, 8)
    };
    u64::from_str_radix(core::str::from_utf8(d).map_err(|_| 22)?, r).map_err(|_| 22)
}

/// Parses a dirfd: `AT_FDCWD` → `None`, otherwise a decimal integer.
pub fn parse_dirfd(s: &CStr) -> Result<Option<DupFd>, i32> {
    let b = s.to_bytes();
    if b == b"AT_FDCWD" {
        Ok(None)
    } else {
        DupFd::from_bytes(b).map(Some)
    }
}

/// Parses resolve flags: `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS` or raw hex.
pub fn parse_resolve_flags(s: &CStr) -> Result<u64, i32> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let h = core::str::from_utf8(b.get(2..).ok_or(22)?).map_err(|_| 22)?;
        u64::from_str_radix(h, 16).map_err(|_| 22)
    } else {
        b.split(|&c| c == b'|').try_fold(0, |acc, name| {
            let v = match name {
                b"RESOLVE_NO_SYMLINKS" => RESOLVE_NO_SYMLINKS,
                b"RESOLVE_NO_MAGICLINKS" => RESOLVE_NO_MAGICLINKS,
                b"RESOLVE_NO_XDEV" => RESOLVE_NO_XDEV,
                b"RESOLVE_BENEATH" => RESOLVE_BENEATH,
                b"RESOLVE_IN_ROOT" => RESOLVE_IN_ROOT,
                b"RESOLVE_CACHED" => RESOLVE_CACHED,
                _ => return Err(22),
            };
            Ok(acc | v)
        })
    }
}
