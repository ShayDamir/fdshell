use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::fcntl::{
    O_APPEND, O_CLOEXEC, O_CREAT, O_DIRECTORY, O_DSYNC, O_EXCL, O_NOCTTY, O_NOFOLLOW, O_NONBLOCK,
    O_RDONLY, O_RDWR, O_SYNC, O_TRUNC, O_WRONLY,
};

use crate::error::FlagParseError;

pub(crate) fn parse_open_flags(s: &CStr) -> Result<i32, Report<FlagParseError>> {
    let b = s.to_bytes();
    if b.starts_with(b"0x") {
        let bytes = b.get(2..).ok_or(FlagParseError::Unknown)?;
        let h = core::str::from_utf8(bytes).change_context(FlagParseError::Utf8)?;
        i32::from_str_radix(h, 16).change_context(FlagParseError::HexParse)
    } else {
        b.split(|&c| c == b'|')
            .try_fold(0, |acc, name| {
                let v = match name {
                    b"O_RDONLY" => O_RDONLY,
                    b"O_WRONLY" => O_WRONLY,
                    b"O_RDWR" => O_RDWR,
                    b"O_CREAT" => O_CREAT,
                    b"O_EXCL" => O_EXCL,
                    b"O_NOCTTY" => O_NOCTTY,
                    b"O_TRUNC" => O_TRUNC,
                    b"O_APPEND" => O_APPEND,
                    b"O_NONBLOCK" => O_NONBLOCK,
                    b"O_DSYNC" => O_DSYNC,
                    b"O_DIRECTORY" => O_DIRECTORY,
                    b"O_NOFOLLOW" => O_NOFOLLOW,
                    b"O_CLOEXEC" => O_CLOEXEC,
                    b"O_SYNC" => O_SYNC,
                    _ => return Err(FlagParseError::Unknown),
                };
                Ok(acc | v)
            })
            .change_context(FlagParseError::Unknown)
    }
}
