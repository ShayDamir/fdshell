mod flags;

use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};
use sys::ImportedFd;

use crate::error::{BuiltinError, Suggestion};

pub struct UnlinkatConfig<'a> {
    pub dirfd: Option<ImportedFd>,
    pub path: &'a CStr,
    pub flags: i32,
}

/// Parses unlinkat CLI arguments into an [`UnlinkatConfig`].
///
/// Returns:
/// - `Err(BuiltinError::Help)` -- `--help` or `-h` was passed
/// - `Err(BuiltinError::InvalidArgument(_))` -- bad flag name, missing value, etc.
pub fn unlinkat_parse<'a>(args: &[&'a CStr]) -> Result<UnlinkatConfig<'a>, Report<BuiltinError>> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        bail!(BuiltinError::Help);
    }

    let mut dirfd = None;
    let mut flags = 0i32;
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
            b"--flags" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                flags = crate::unlinkat::parse::flags::parse_unlink_flags(s)
                    .change_context(BuiltinError::InvalidArgument("flags"))
                    .attach_opaque(Suggestion(
                        "Use AT_REMOVEDIR to remove a directory instead of a file, or 0x0",
                    ))?;
            }
            a if a.starts_with(b"-") => {
                bail!(BuiltinError::InvalidArgument("flag"));
            }
            _ => {
                if path.is_some() {
                    bail!(BuiltinError::InvalidArgument("path"));
                }
                path = Some(arg);
            }
        }
    }

    let path = path.ok_or(BuiltinError::InvalidArgument("path"))?;
    if path.to_bytes().is_empty() {
        bail!(BuiltinError::InvalidArgument("path"));
    }

    Ok(UnlinkatConfig { dirfd, path, flags })
}
