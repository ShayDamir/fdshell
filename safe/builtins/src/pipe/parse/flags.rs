use core::ffi::CStr;
use sys::fcntl::{O_DIRECT, O_NONBLOCK};

use crate::error::BuiltinError;

pub(crate) fn parse_pipe_flag(s: &CStr) -> Result<i32, BuiltinError> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let bytes = b.get(2..).ok_or(BuiltinError::InvalidArgument)?;
        let h = match core::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return Err(BuiltinError::InvalidArgument),
        };
        match i32::from_str_radix(h, 16) {
            Ok(v) => Ok(v),
            Err(_) => Err(BuiltinError::InvalidArgument),
        }
    } else {
        match b {
            b"O_NONBLOCK" => Ok(O_NONBLOCK),
            b"O_DIRECT" => Ok(O_DIRECT),
            _ => Err(BuiltinError::InvalidArgument),
        }
    }
}
