use core::ffi::CStr;
use sys::ImportedFd;

pub fn wants_help(args: &[&CStr]) -> bool {
    args.iter()
        .any(|a| a.to_bytes() == b"--help" || a.to_bytes() == b"-h")
}

pub fn split(arg: &CStr) -> Result<(&[u8], Option<&CStr>), i32> {
    let bytes = arg.to_bytes_with_nul();
    if let Some(eq) = bytes.iter().position(|&c| c == b'=') {
        let key = bytes.get(..eq).ok_or(sys::errno::EINVAL)?;
        let val_bytes = bytes.get(eq + 1..).ok_or(sys::errno::EINVAL)?;
        let val = match CStr::from_bytes_with_nul(val_bytes) {
            Ok(c) => c,
            Err(_) => return Err(sys::errno::EINVAL),
        };
        Ok((key, Some(val)))
    } else {
        let key = bytes.strip_suffix(b"\0").ok_or(sys::errno::EINVAL)?;
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
            let v = args.get(*i).ok_or(sys::errno::EINVAL)?;
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
    let s = match core::str::from_utf8(d) {
        Ok(s) => s,
        Err(_) => return Err(sys::errno::EINVAL),
    };
    match u64::from_str_radix(s, r) {
        Ok(v) => Ok(v),
        Err(_) => Err(sys::errno::EINVAL),
    }
}

pub fn parse_dirfd(s: &CStr) -> Result<Option<ImportedFd>, i32> {
    if s == c"AT_FDCWD" {
        Ok(None)
    } else {
        Ok(ImportedFd::try_from(s).map(Some)?)
    }
}
