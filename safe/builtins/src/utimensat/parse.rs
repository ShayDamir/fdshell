mod flags;
mod time;

pub use time::TimeSpec;

use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};
use sys::ImportedFd;

use crate::error::{BuiltinError, Suggestion};

pub struct UtimensatConfig<'a> {
    pub dirfd: Option<ImportedFd>,
    pub atime: TimeSpec,
    pub mtime: TimeSpec,
    pub flags: i32,
    pub path: &'a CStr,
}

/// Parses utimensat CLI arguments into an [`UtimensatConfig`].
///
/// Returns:
/// - `Err(BuiltinError::Help)` -- `--help` or `-h` was passed
/// - `Err(BuiltinError::InvalidArgument(_))` -- bad flag name, missing value, etc.
pub fn utimensat_parse<'a>(args: &[&'a CStr]) -> Result<UtimensatConfig<'a>, Report<BuiltinError>> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        bail!(BuiltinError::Help);
    }

    let mut dirfd = None;
    let mut atime = TimeSpec::Now;
    let mut mtime = TimeSpec::Now;
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
            b"--atime" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                atime = time::parse_time_spec(s)
                    .change_context(BuiltinError::InvalidArgument("atime"))?;
            }
            b"--mtime" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                mtime = time::parse_time_spec(s)
                    .change_context(BuiltinError::InvalidArgument("mtime"))?;
            }
            b"--flags" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                flags = crate::utimensat::parse::flags::parse_utime_flags(s)
                    .change_context(BuiltinError::InvalidArgument("flags"))
                    .attach_opaque(Suggestion(
                        "Use AT_SYMLINK_NOFOLLOW to act on the link itself, or 0x0",
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

    Ok(UtimensatConfig {
        dirfd,
        atime,
        mtime,
        flags,
        path,
    })
}
