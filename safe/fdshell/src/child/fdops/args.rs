//! Shared positional-argument parsing for the fd-ops builtins and the
//! per-builtin config structs.

use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail, ensure};
use sys::ShortCStr;

use builtins::error::BuiltinError;

#[cfg_attr(test, derive(Debug))]
pub(crate) struct LseekConfig {
    pub(crate) var: ShortCStr,
    pub(crate) offset: i64,
    pub(crate) whence: i32,
}

#[cfg_attr(test, derive(Debug))]
pub(crate) struct FtruncateConfig {
    pub(crate) var: ShortCStr,
    pub(crate) length: Option<i64>,
}

#[cfg_attr(test, derive(Debug))]
pub(crate) struct FsyncConfig {
    pub(crate) var: ShortCStr,
}

pub(crate) fn var_arg(args: &[ShortCStr]) -> Result<ShortCStr, Report<BuiltinError>> {
    let first = args
        .first()
        .ok_or(BuiltinError::MissingArgument("fd var"))?;
    let name = first
        .strip_prefix(b"%")
        .ok_or(BuiltinError::InvalidArgument("fd var"))?;
    ensure!(
        !name.contains(b'%'),
        BuiltinError::InvalidArgument("fd var")
    );
    Ok(name)
}

pub(crate) fn number(s: &CStr, what: &'static str) -> Result<i64, Report<BuiltinError>> {
    let text =
        core::str::from_utf8(s.to_bytes()).change_context(BuiltinError::InvalidArgument(what))?;
    text.parse::<i64>()
        .change_context(BuiltinError::InvalidArgument(what))
}

pub(crate) fn length(s: &CStr) -> Result<i64, Report<BuiltinError>> {
    let n = number(s, "length")?;
    if n < 0 {
        bail!(BuiltinError::InvalidArgument("length"));
    }
    Ok(n)
}

pub(crate) fn whence(s: &CStr) -> Result<i32, Report<BuiltinError>> {
    match s.to_bytes() {
        b"0" => Ok(sys::fcntl::SEEK_SET),
        b"1" => Ok(sys::fcntl::SEEK_CUR),
        b"2" => Ok(sys::fcntl::SEEK_END),
        _ => bail!(BuiltinError::InvalidArgument("whence")),
    }
}

pub(crate) fn no_extra(actual: usize, max: usize) -> Result<(), Report<BuiltinError>> {
    if actual > max {
        bail!(BuiltinError::InvalidArgument("arg"));
    }
    Ok(())
}
