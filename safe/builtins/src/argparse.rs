use core::ffi::CStr;
use sys::ImportedFd;
use sys::errno::EINVAL;

pub fn wants_help(args: &[&CStr]) -> bool {
    args.iter()
        .any(|a| a.to_bytes() == b"--help" || a.to_bytes() == b"-h")
}

pub fn split(arg: &CStr) -> Result<(&[u8], Option<&CStr>), i32> {
    let bytes = arg.to_bytes_with_nul();
    if let Some(eq) = bytes.iter().position(|&c| c == b'=') {
        let key = bytes.get(..eq).ok_or(EINVAL)?;
        let val =
            CStr::from_bytes_with_nul(bytes.get(eq + 1..).ok_or(EINVAL)?).map_err(|_| EINVAL)?;
        Ok((key, Some(val)))
    } else {
        let key = bytes.strip_suffix(b"\0").ok_or(EINVAL)?;
        Ok((key, None))
    }
}

pub fn next_val<'a>(
    args: &[&'a CStr],
    i: &mut usize,
    val: Option<&'a CStr>,
) -> Result<&'a CStr, i32> {
    match val {
        Some(v) => Ok(v),
        None => {
            let v = args.get(*i).ok_or(EINVAL)?;
            *i += 1;
            Ok(v)
        }
    }
}

pub fn parse_mode(s: &CStr) -> Result<u64, i32> {
    let b = s.to_bytes();
    let (d, r) = if let Some(h) = b.strip_prefix(b"0x") {
        (h, 16)
    } else if let Some(o) = b.strip_prefix(b"0o") {
        (o, 8)
    } else {
        (b, 8)
    };
    u64::from_str_radix(core::str::from_utf8(d).map_err(|_| EINVAL)?, r).map_err(|_| EINVAL)
}

pub fn parse_dirfd(s: &CStr) -> Result<Option<ImportedFd>, i32> {
    if s == c"AT_FDCWD" {
        Ok(None)
    } else {
        Ok(ImportedFd::try_from(s).map(Some)?)
    }
}
