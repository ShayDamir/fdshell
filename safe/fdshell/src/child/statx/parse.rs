//! `statx` argument parsing. The path or `%var` comes from the first token
//! (`args`, originals); the flags come from `args` too, because the `--dir`
//! value is an fd-variable reference that must stay `%d` (in `refs` it would
//! already be substituted to the fd number).

use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail, ensure};
use sys::ShortCStr;

use builtins::error::BuiltinError;

use crate::child::fdops::args::var_arg;

#[cfg_attr(test, derive(Debug))]
pub(crate) enum Target<'a> {
    /// `statx PATH [--dir %d] [--nofollow]`
    Path {
        path: &'a CStr,
        dir: Option<ShortCStr>,
        nofollow: bool,
    },
    /// `statx %fd [--nofollow]` — re-stat the open handle.
    Fd { var: ShortCStr, nofollow: bool },
}

pub(crate) fn statx_parse<'a>(
    refs: &[&'a CStr],
    args: &[ShortCStr],
) -> Result<Target<'a>, Report<BuiltinError>> {
    if builtins::argparse::wants_help(refs) {
        bail!(BuiltinError::Help);
    }
    let first = args.first().ok_or(BuiltinError::MissingArgument("path"))?;
    let (nofollow, dir) = parse_flags(args)?;
    if first.starts_with(b"%") {
        ensure!(dir.is_none(), BuiltinError::InvalidArgument("--dir"));
        let var = var_arg(args)?;
        return Ok(Target::Fd { var, nofollow });
    }
    let path = refs.first().ok_or(BuiltinError::MissingArgument("path"))?;
    ensure!(
        !path.to_bytes().is_empty(),
        BuiltinError::InvalidArgument("path")
    );
    Ok(Target::Path {
        path,
        dir,
        nofollow,
    })
}

/// The flags after the first token: `--nofollow` and `--dir %d`.
fn parse_flags(args: &[ShortCStr]) -> Result<(bool, Option<ShortCStr>), Report<BuiltinError>> {
    let mut nofollow = false;
    let mut dir: Option<ShortCStr> = None;
    let mut i = 1;
    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument("arg"))?;
        let (key, val) = split_eq(arg)?;
        i += 1;
        match key {
            b"--nofollow" => {
                ensure!(val.is_none(), BuiltinError::InvalidArgument("--nofollow"));
                nofollow = true;
            }
            b"--dir" => {
                ensure!(dir.is_none(), BuiltinError::InvalidArgument("--dir"));
                dir = Some(match val {
                    Some(inline) => dir_var(inline)?,
                    None => {
                        let v = args.get(i).ok_or(BuiltinError::InvalidArgument("--dir"))?;
                        i += 1;
                        dir_var(v.as_bytes().change_context(BuiltinError::Never)?)?
                    }
                });
            }
            _ => bail!(BuiltinError::InvalidArgument("flag")),
        }
    }
    Ok((nofollow, dir))
}

/// Split `--key=value` into `("key", Some("value"))`, else `("key", None)`.
type KeyVal<'a> = (&'a [u8], Option<&'a [u8]>);

fn split_eq(arg: &ShortCStr) -> Result<KeyVal<'_>, Report<BuiltinError>> {
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
fn dir_var(v: &[u8]) -> Result<ShortCStr, Report<BuiltinError>> {
    let name = v
        .strip_prefix(b"%")
        .ok_or(BuiltinError::InvalidArgument("dir var"))?;
    ensure!(
        !name.is_empty() && !name.contains(&b'%'),
        BuiltinError::InvalidArgument("dir var")
    );
    ShortCStr::from_vec(name.to_vec()).change_context(BuiltinError::Never)
}
