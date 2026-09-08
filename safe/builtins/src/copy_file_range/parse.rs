//! `copy_file_range` argument parsing: `copy_file_range %in %out [COUNT]`.
//!
//! Both operands are `%var` fd variables taken from `args` (the originals, so
//! the `%` is intact); `COUNT` is an optional byte count. There are no flags,
//! so `--help`/`-h` (in `refs`, the substituted command line) is the only
//! special case, and any other token is rejected as a stray argument.

use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail, ensure};

use crate::argparse;
use crate::error::BuiltinError;

use sys::ShortCStr;

#[cfg_attr(test, derive(Debug))]
pub struct CopyFileRangeConfig {
    pub in_var: ShortCStr,
    pub out_var: ShortCStr,
    pub count: Option<u64>,
}

pub fn copy_file_range_parse(
    refs: &[&CStr],
    args: &[ShortCStr],
) -> Result<CopyFileRangeConfig, Report<BuiltinError>> {
    if argparse::wants_help(refs) {
        bail!(BuiltinError::Help);
    }

    let in_var = fd_var(
        args.first()
            .ok_or(BuiltinError::MissingArgument("in fd var"))?,
        "in fd var",
    )?;
    let out_var = fd_var(
        args.get(1)
            .ok_or(BuiltinError::MissingArgument("out fd var"))?,
        "out fd var",
    )?;
    let count = match args.get(2) {
        Some(arg) => {
            ensure!(args.get(3).is_none(), BuiltinError::InvalidArgument("arg"));
            Some(parse_count(arg)?)
        }
        None => None,
    };

    Ok(CopyFileRangeConfig {
        in_var,
        out_var,
        count,
    })
}

/// Strip the leading `%` from a `%var` token, rejecting an empty name or a
/// name containing another `%` (which could name more than one variable).
fn fd_var(arg: &ShortCStr, what: &'static str) -> Result<ShortCStr, Report<BuiltinError>> {
    let name = arg
        .strip_prefix(b"%")
        .ok_or(BuiltinError::InvalidArgument(what))?;
    let bytes = name
        .as_bytes()
        .change_context(BuiltinError::InvalidArgument(what))?;
    ensure!(!bytes.is_empty(), BuiltinError::InvalidArgument(what));
    ensure!(!bytes.contains(&b'%'), BuiltinError::InvalidArgument(what));
    Ok(name)
}

/// A non-negative byte count, in decimal.
fn parse_count(s: &ShortCStr) -> Result<u64, Report<BuiltinError>> {
    let bytes = s
        .as_bytes()
        .change_context(BuiltinError::InvalidArgument("count"))?;
    let text =
        core::str::from_utf8(bytes).change_context(BuiltinError::InvalidArgument("count"))?;
    text.parse::<u64>()
        .change_context(BuiltinError::InvalidArgument("count"))
}
