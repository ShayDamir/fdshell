use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::ImportedFd;

use crate::error::{BuiltinError, Suggestion};

pub struct MkdiratConfig<'a> {
    pub dirfd: Option<ImportedFd>,
    pub path: &'a CStr,
    pub mode: u32,
    pub resolve: u64,
}

/// Parses mkdirat CLI arguments into an [`MkdiratConfig`].
///
/// Returns:
/// - `Err(BuiltinError::Help)` -- `--help` or `-h` was passed
/// - `Err(BuiltinError::InvalidArgument(_))` -- bad flag name, missing value, etc.
pub fn mkdirat_parse<'a>(args: &[&'a CStr]) -> Result<MkdiratConfig<'a>, Report<BuiltinError>> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        return Err(Report::new(BuiltinError::Help));
    }

    let mut dirfd = None;
    let mut mode: u32 = 0;
    let mut resolve: u64 = 0;
    let mut path: Option<&'a CStr> = None;
    let mut i = 0;

    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument("arg"))?;
        i += 1;
        let (key, val) = crate::argparse::split(arg)?;
        match key {
            b"--dirfd" => {
                dirfd = crate::argparse::parse_dirfd(crate::argparse::next_val(args, &mut i, val)?)?
            }
            b"--mode" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                mode = crate::argparse::parse_mode(s)
                    .change_context(BuiltinError::InvalidArgument("mode"))
                    .attach_opaque(Suggestion(
                        "Use octal without prefix (e.g. 755) or hex with 0x prefix (e.g. 0x1ff)",
                    ))? as u32;
            }
            b"--resolve" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                resolve = crate::resolve::parse_resolve_flags(s)
                    .change_context(BuiltinError::InvalidArgument("resolve"))
                    .attach_opaque(Suggestion(
                        "Use RESOLVE_NO_SYMLINKS, RESOLVE_NO_MAGICLINKS, RESOLVE_NO_XDEV, \
                        RESOLVE_BENEATH, RESOLVE_IN_ROOT, RESOLVE_CACHED, or a hex value (e.g. 0x80000)",
                    ))?;
            }
            a if a.starts_with(b"-") => {
                return Err(Report::new(BuiltinError::InvalidArgument("flag")));
            }
            _ => {
                if path.is_some() {
                    return Err(Report::new(BuiltinError::InvalidArgument("path")));
                }
                path = Some(arg);
            }
        }
    }

    let path = path.ok_or(BuiltinError::InvalidArgument("path"))?;
    if path.to_bytes().is_empty() {
        return Err(Report::new(BuiltinError::InvalidArgument("path")));
    }

    Ok(MkdiratConfig {
        dirfd,
        path,
        mode,
        resolve,
    })
}
