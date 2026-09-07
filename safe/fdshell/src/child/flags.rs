//! Shared flag-value parsing for the forked builtins. `--dir %d` is the
//! common one: its value is an fd-variable reference and must be read from
//! `args` (originals), where `%d` has not yet been substituted to the fd
//! number it carries in `refs`.

use error_stack::{Report, ResultExt, ensure};
use sys::ShortCStr;

use builtins::error::BuiltinError;

/// Split `--key=value` into `("key", Some("value"))`, else `("key", None)`.
pub(crate) type KeyVal<'a> = (&'a [u8], Option<&'a [u8]>);

pub(crate) fn split_eq(arg: &ShortCStr) -> Result<KeyVal<'_>, Report<BuiltinError>> {
    let bytes = arg.as_bytes().change_context(BuiltinError::Never)?;
    match bytes.iter().position(|&b| b == b'=') {
        Some(eq) => Ok((
            bytes.get(..eq).ok_or(BuiltinError::Never)?,
            Some(bytes.get(eq + 1..).ok_or(BuiltinError::Never)?),
        )),
        None => Ok((bytes, None)),
    }
}

/// `--dir` takes an fd variable: `%name` with no further `%`.
pub(crate) fn dir_var(v: &[u8]) -> Result<ShortCStr, Report<BuiltinError>> {
    let name = v
        .strip_prefix(b"%")
        .ok_or(BuiltinError::InvalidArgument("dir var"))?;
    ensure!(
        !name.is_empty() && !name.contains(&b'%'),
        BuiltinError::InvalidArgument("dir var")
    );
    ShortCStr::from_vec(name.to_vec()).change_context(BuiltinError::Never)
}

#[cfg(test)]
mod tests;
