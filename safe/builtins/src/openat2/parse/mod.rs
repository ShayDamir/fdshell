mod flags;

use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::ImportedFd;
use sys::openat2::OpenHow;

use crate::error::{BuiltinError, Suggestion};

pub struct Openat2Config<'a> {
    pub dirfd: Option<ImportedFd>,
    pub path: &'a CStr,
    pub how: OpenHow,
}

/// Parses openat2 CLI arguments into an [`Openat2Config`].
pub fn openat2_parse<'a>(args: &[&'a CStr]) -> Result<Openat2Config<'a>, Report<BuiltinError>> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        return Err(Report::new(BuiltinError::Help));
    }

    let mut dirfd = None;
    let mut open_flags = 0;
    let mut mode: u64 = 0;
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
            b"--flags" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                open_flags |= flags::parse_open_flags(s)
                    .change_context(BuiltinError::InvalidArgument("flags"))
                    .attach_opaque(Suggestion(
                        "Use O_RDONLY, O_WRONLY, O_CREAT, O_EXCL, O_NOCTTY, O_TRUNC, O_APPEND, \
                        O_NONBLOCK, O_DSYNC, O_DIRECTORY, O_NOFOLLOW, O_CLOEXEC, O_SYNC, or a \
                        hex value (e.g. 0x4000)",
                    ))?;
            }
            b"--mode" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                mode = crate::argparse::parse_mode(s)
                    .change_context(BuiltinError::InvalidArgument("mode"))
                    .attach_opaque(Suggestion(
                        "Use octal without prefix (e.g. 755) or hex with 0x prefix (e.g. 0x1ff)",
                    ))?;
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

    Ok(Openat2Config {
        dirfd,
        path,
        how: OpenHow {
            flags: open_flags as u64,
            mode,
            resolve,
        },
    })
}
