use core::ffi::CStr;
use sys::errno::EINVAL;
use sys::fcntl::{O_DIRECT, O_NONBLOCK};

pub(crate) fn parse_pipe_flag(s: &CStr) -> Result<i32, i32> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let h = core::str::from_utf8(b.get(2..).ok_or(EINVAL)?).map_err(|_| EINVAL)?;
        i32::from_str_radix(h, 16).map_err(|_| EINVAL)
    } else {
        match b {
            b"O_NONBLOCK" => Ok(O_NONBLOCK),
            b"O_DIRECT" => Ok(O_DIRECT),
            _ => Err(EINVAL),
        }
    }
}
