use core::ffi::CStr;
use error_stack::{Report, bail};
use sys::ImportedFd;

use crate::error::BuiltinError;

pub struct SymlinkatConfig<'a> {
    pub dirfd: Option<ImportedFd>,
    pub target: &'a CStr,
    pub linkpath: &'a CStr,
}

/// Parses symlinkat CLI arguments into an [`SymlinkatConfig`].
///
/// Returns:
/// - `Err(BuiltinError::Help)` -- `--help` or `-h` was passed
/// - `Err(BuiltinError::InvalidArgument(_))` -- bad flag name, missing value, etc.
pub fn symlinkat_parse<'a>(
    args: &[&'a CStr],
    strict: bool,
) -> Result<SymlinkatConfig<'a>, Report<BuiltinError>> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        bail!(BuiltinError::Help);
    }

    let mut dirfd = None;
    let mut target: Option<&'a CStr> = None;
    let mut linkpath: Option<&'a CStr> = None;
    let mut i = 0;

    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument("arg"))?;
        i += 1;
        let (key, val) = crate::argparse::split(arg)?;
        match key {
            b"--dirfd" => {
                dirfd = crate::argparse::parse_dirfd(crate::argparse::next_val(args, &mut i, val)?)?
            }
            a if a.starts_with(b"-") => {
                bail!(BuiltinError::InvalidArgument("flag"));
            }
            _ => {
                if target.is_none() {
                    target = Some(arg);
                } else if linkpath.is_none() {
                    linkpath = Some(arg);
                } else {
                    bail!(BuiltinError::InvalidArgument("arg"));
                }
            }
        }
    }

    let target = target.ok_or(BuiltinError::InvalidArgument("target"))?;
    let linkpath = linkpath.ok_or(BuiltinError::InvalidArgument("linkpath"))?;
    if target.to_bytes().is_empty() {
        bail!(BuiltinError::InvalidArgument("target"));
    }
    if linkpath.to_bytes().is_empty() {
        bail!(BuiltinError::InvalidArgument("linkpath"));
    }

    // Only the link location is constrained; `target` is link content, stored
    // verbatim and never resolved, so an absolute target is allowed.
    crate::strict::require_dirfd(strict, dirfd.as_ref())?;
    crate::strict::require_relative(strict, linkpath)?;

    Ok(SymlinkatConfig {
        dirfd,
        target,
        linkpath,
    })
}
