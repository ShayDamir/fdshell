use core::ffi::CStr;

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
        let val = CStr::from_bytes_with_nul(bytes.get(eq + 1..).ok_or(22)?)
            .map_err(|_| 22)?;
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
