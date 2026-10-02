//! Per-builtin argument parsing for `lseek`, `ftruncate`, `fsync`,
//! `fallocate`. Numbers come from `refs` (substituted); the `%var` comes from
//! `args` (original).

use core::ffi::CStr;
use error_stack::{Report, bail, ensure};
use sys::ShortCStr;

use builtins::error::BuiltinError;

use super::args::{
    FallocateConfig, FsyncConfig, FtruncateConfig, LseekConfig, length, no_extra, number, var_arg,
    whence,
};

/// `bail!(Help)` when `refs` asks for help.
fn check_help(refs: &[&CStr]) -> Result<(), Report<BuiltinError>> {
    if builtins::argparse::wants_help(refs) {
        bail!(BuiltinError::Help)
    }
    Ok(())
}

/// The required numeric argument at `refs[idx]`.
fn number_at(refs: &[&CStr], idx: usize, what: &'static str) -> Result<i64, Report<BuiltinError>> {
    match refs.get(idx) {
        Some(v) => number(v, what),
        None => bail!(BuiltinError::MissingArgument(what)),
    }
}

pub(crate) fn lseek_parse(
    refs: &[&CStr],
    args: &[ShortCStr],
) -> Result<LseekConfig, Report<BuiltinError>> {
    check_help(refs)?;
    let var = var_arg(args)?;
    let offset = number_at(refs, 1, "offset")?;
    let whence = refs
        .get(2)
        .map(|w| whence(w))
        .transpose()?
        .unwrap_or(sys::fcntl::SEEK_SET);
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
    check_help(refs)?;
    let var = var_arg(args)?;
    let length = refs.get(1).map(|l| length(l)).transpose()?;
    no_extra(refs.len(), 2)?;
    Ok(FtruncateConfig { var, length })
}

pub(crate) fn fsync_parse(
    refs: &[&CStr],
    args: &[ShortCStr],
) -> Result<FsyncConfig, Report<BuiltinError>> {
    check_help(refs)?;
    let var = var_arg(args)?;
    no_extra(refs.len(), 1)?;
    Ok(FsyncConfig { var })
}

pub(crate) fn fallocate_parse(
    refs: &[&CStr],
    args: &[ShortCStr],
) -> Result<FallocateConfig, Report<BuiltinError>> {
    check_help(refs)?;
    let var = var_arg(args)?;
    let offset = number_at(refs, 1, "offset")?;
    ensure!(offset >= 0, BuiltinError::InvalidArgument("offset"));
    let len = number_at(refs, 2, "len")?;
    ensure!(len > 0, BuiltinError::InvalidArgument("len"));
    no_extra(refs.len(), 3)?;
    Ok(FallocateConfig { var, offset, len })
}
