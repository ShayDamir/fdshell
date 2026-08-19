use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};
use sys::ImportedFd;

use crate::argparse;
use crate::error::{BuiltinError, Suggestion};
use crate::resolve::parse_resolve_flags;

use super::flags;

#[derive(Default)]
pub(crate) struct Acc<'a> {
    pub(crate) dirfd: Option<ImportedFd>,
    pub(crate) open_flags: i32,
    pub(crate) mode: u64,
    pub(crate) resolve: u64,
    pub(crate) path: Option<&'a CStr>,
}

pub(crate) fn parse_arg<'a>(
    acc: &mut Acc<'a>,
    args: &[&'a CStr],
    i: &mut usize,
    arg: &'a CStr,
) -> Result<(), Report<BuiltinError>> {
    let (key, val) = argparse::split(arg)?;
    match key {
        b"--dirfd" => acc.dirfd = argparse::parse_dirfd(argparse::next_val(args, i, val)?)?,
        b"--flags" => {
            acc.open_flags |= flags::parse_open_flags(argparse::next_val(args, i, val)?)
                .change_context(BuiltinError::InvalidArgument("flags"))
                .attach_opaque(Suggestion(
                    "Use O_RDONLY, O_WRONLY, O_CREAT, O_EXCL, O_NOCTTY, O_TRUNC, O_APPEND, \
                    O_NONBLOCK, O_DSYNC, O_DIRECTORY, O_NOFOLLOW, O_CLOEXEC, O_SYNC, or a \
                    hex value (e.g. 0x4000)",
                ))?;
        }
        b"--mode" => {
            acc.mode = argparse::parse_mode(argparse::next_val(args, i, val)?)
                .change_context(BuiltinError::InvalidArgument("mode"))
                .attach_opaque(Suggestion(
                    "Use octal without prefix (e.g. 755) or hex with 0x prefix (e.g. 0x1ff)",
                ))? as u64;
        }
        b"--resolve" => {
            acc.resolve = parse_resolve_flags(argparse::next_val(args, i, val)?)
                .change_context(BuiltinError::InvalidArgument("resolve"))
                .attach_opaque(Suggestion(
                    "Use RESOLVE_NO_SYMLINKS, RESOLVE_NO_MAGICLINKS, RESOLVE_NO_XDEV, \
                    RESOLVE_BENEATH, RESOLVE_IN_ROOT, RESOLVE_CACHED, or a hex value (e.g. 0x80000)",
                ))?;
        }
        a if a.starts_with(b"-") => {
            bail!(BuiltinError::InvalidArgument("flag"));
        }
        _ => {
            if acc.path.is_some() {
                bail!(BuiltinError::InvalidArgument("path"));
            }
            acc.path = Some(arg);
        }
    }
    Ok(())
}
