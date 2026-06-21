use core::ffi::CStr;
use sys::fcntl::{O_DIRECT, O_NONBLOCK};

pub(crate) fn parse_pipe_flag(s: &CStr) -> Result<i32, i32> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let bytes = b.get(2..).ok_or(sys::errno::EINVAL)?;
        let h = match core::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return Err(sys::errno::EINVAL),
        };
        match i32::from_str_radix(h, 16) {
            Ok(v) => Ok(v),
            Err(_) => Err(sys::errno::EINVAL),
        }
    } else {
        match b {
            b"O_NONBLOCK" => Ok(O_NONBLOCK),
            b"O_DIRECT" => Ok(O_DIRECT),
            _ => Err(sys::errno::EINVAL),
        }
    }
}
