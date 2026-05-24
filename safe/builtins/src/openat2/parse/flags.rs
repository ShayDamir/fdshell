use core::ffi::CStr;

pub(crate) fn parse_open_flags(s: &CStr) -> Result<i32, i32> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let h = core::str::from_utf8(b.get(2..).ok_or(22)?).map_err(|_| 22)?;
        i32::from_str_radix(h, 16).map_err(|_| 22)
    } else {
        b.split(|&c| c == b'|').try_fold(0, |acc, name| {
            let v = match name {
                b"O_RDONLY" => 0,
                b"O_WRONLY" => 1,
                b"O_RDWR" => 2,
                b"O_CREAT" => 0o100,
                b"O_EXCL" => 0o200,
                b"O_NOCTTY" => 0o400,
                b"O_TRUNC" => 0o1000,
                b"O_APPEND" => 0o2000,
                b"O_NONBLOCK" => 0o4000,
                b"O_DSYNC" => 0o10000,
                b"O_DIRECTORY" => 0o200000,
                b"O_NOFOLLOW" => 0o400000,
                b"O_CLOEXEC" => 0o20000000,
                b"O_SYNC" => 0o4010000,
                _ => return Err(22),
            };
            Ok(acc | v)
        })
    }
}

pub(crate) fn parse_resolve_flags(s: &CStr) -> Result<u64, i32> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let h = core::str::from_utf8(b.get(2..).ok_or(22)?).map_err(|_| 22)?;
        u64::from_str_radix(h, 16).map_err(|_| 22)
    } else {
        b.split(|&c| c == b'|').try_fold(0, |acc, name| {
            let v = match name {
                b"RESOLVE_NO_SYMLINKS" => 1,
                b"RESOLVE_NO_MAGICLINKS" => 2,
                b"RESOLVE_NO_XDEV" => 4,
                b"RESOLVE_BENEATH" => 8,
                b"RESOLVE_IN_ROOT" => 16,
                b"RESOLVE_CACHED" => 32,
                _ => return Err(22),
            };
            Ok(acc | v)
        })
    }
}
