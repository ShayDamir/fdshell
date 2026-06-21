use core::ffi::CStr;

pub(crate) fn parse_rename_flags(s: &CStr) -> Result<u32, i32> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let bytes = b.get(2..).ok_or(sys::errno::EINVAL)?;
        let h = match core::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return Err(sys::errno::EINVAL),
        };
        match u32::from_str_radix(h, 16) {
            Ok(v) => Ok(v),
            Err(_) => Err(sys::errno::EINVAL),
        }
    } else {
        b.split(|&c| c == b'|').try_fold(0, |acc, name| {
            let v = match name {
                b"RENAME_NOREPLACE" => sys::renameat2::RENAME_NOREPLACE,
                b"RENAME_EXCHANGE" => sys::renameat2::RENAME_EXCHANGE,
                b"RENAME_WHITEOUT" => sys::renameat2::RENAME_WHITEOUT,
                _ => return Err(sys::errno::EINVAL),
            };
            Ok(acc | v)
        })
    }
}
