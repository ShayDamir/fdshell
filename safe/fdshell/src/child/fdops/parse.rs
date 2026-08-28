//! Per-builtin argument parsing for `lseek`, `ftruncate`, `fsync`. Numbers
//! come from `refs` (substituted); the `%var` comes from `args` (original).

use core::ffi::CStr;
use error_stack::{Report, bail};
use sys::ShortCStr;

use builtins::error::BuiltinError;

use super::args::{
    FsyncConfig, FtruncateConfig, LseekConfig, length, no_extra, number, var_arg, whence,
};

pub(crate) fn lseek_parse(
    refs: &[&CStr],
    args: &[ShortCStr],
) -> Result<LseekConfig, Report<BuiltinError>> {
    if builtins::argparse::wants_help(refs) {
        bail!(BuiltinError::Help);
    }
    let var = var_arg(args)?;
    let offset = match refs.get(1) {
        Some(o) => number(o, "offset")?,
        None => bail!(BuiltinError::MissingArgument("offset")),
    };
    let whence = match refs.get(2) {
        Some(w) => whence(w)?,
        None => sys::fcntl::SEEK_SET,
    };
    no_extra(refs.len(), 3)?;
    Ok(LseekConfig {
        var,
        offset,
        whence,
    })
}

pub(crate) fn ftruncate_parse(
    refs: &[&CStr],
    args: &[ShortCStr],
) -> Result<FtruncateConfig, Report<BuiltinError>> {
    if builtins::argparse::wants_help(refs) {
        bail!(BuiltinError::Help);
    }
    let var = var_arg(args)?;
    let length = match refs.get(1) {
        Some(l) => Some(length(l)?),
        None => None,
    };
    no_extra(refs.len(), 2)?;
    Ok(FtruncateConfig { var, length })
}

pub(crate) fn fsync_parse(
    refs: &[&CStr],
    args: &[ShortCStr],
) -> Result<FsyncConfig, Report<BuiltinError>> {
    if builtins::argparse::wants_help(refs) {
        bail!(BuiltinError::Help);
    }
    let var = var_arg(args)?;
    no_extra(refs.len(), 1)?;
    Ok(FsyncConfig { var })
}
