use core::ffi::CStr;
use sys::errno::EINVAL;

pub(crate) fn parse_rename_flags(s: &CStr) -> Result<u32, i32> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let h = core::str::from_utf8(b.get(2..).ok_or(EINVAL)?).map_err(|_| EINVAL)?;
        u32::from_str_radix(h, 16).map_err(|_| EINVAL)
    } else {
        b.split(|&c| c == b'|').try_fold(0, |acc, name| {
            let v = match name {
                b"RENAME_NOREPLACE" => sys::renameat2::RENAME_NOREPLACE,
                b"RENAME_EXCHANGE" => sys::renameat2::RENAME_EXCHANGE,
                b"RENAME_WHITEOUT" => sys::renameat2::RENAME_WHITEOUT,
                _ => return Err(EINVAL),
            };
            Ok(acc | v)
        })
    }
}
