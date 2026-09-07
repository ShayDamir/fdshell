//! `ls` argument parsing. The `%dirfd` or `PATH` comes from the first token
//! (`args`, originals); `--dir %d` stays `%d` in `args` because its value must
//! not be substituted to an fd number before we resolve the fd variable.

use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail, ensure};
use sys::ShortCStr;

use builtins::error::BuiltinError;

use crate::child::fdops::args::var_arg;
use crate::child::flags::{dir_var, split_eq};

#[cfg_attr(test, derive(Debug))]
pub(crate) enum Target<'a> {
    /// `ls %dirfd` — list the directory behind the open handle.
    Fd { var: ShortCStr },
    /// `ls PATH [--dir %d]` — open the path as a directory and list it.
    Path {
        path: &'a CStr,
        dir: Option<ShortCStr>,
    },
}

pub(crate) fn ls_parse<'a>(
    refs: &[&'a CStr],
    args: &[ShortCStr],
) -> Result<Target<'a>, Report<BuiltinError>> {
    if builtins::argparse::wants_help(refs) {
        bail!(BuiltinError::Help);
    }
    let first = args.first().ok_or(BuiltinError::MissingArgument("path"))?;
    let dir = parse_dir(args)?;
    if first.starts_with(b"%") {
        ensure!(dir.is_none(), BuiltinError::InvalidArgument("--dir"));
        let var = var_arg(args)?;
        return Ok(Target::Fd { var });
    }
    let path = refs.first().ok_or(BuiltinError::MissingArgument("path"))?;
    ensure!(
        !path.to_bytes().is_empty(),
        BuiltinError::InvalidArgument("path")
    );
    Ok(Target::Path { path, dir })
}

/// The flags after the first token: `--dir %d` (or `--dir=%d`).
fn parse_dir(args: &[ShortCStr]) -> Result<Option<ShortCStr>, Report<BuiltinError>> {
    let mut dir: Option<ShortCStr> = None;
    let mut i = 1;
    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument("arg"))?;
        let (key, val) = split_eq(arg)?;
        i += 1;
        match key {
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
    Ok(dir)
}
