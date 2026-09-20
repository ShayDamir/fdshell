mod flags;

use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};
use sys::ImportedFd;

use crate::error::{BuiltinError, Suggestion};

pub struct Renameat2Config<'a> {
    pub olddirfd: Option<ImportedFd>,
    pub newdirfd: Option<ImportedFd>,
    pub oldpath: &'a CStr,
    pub newpath: &'a CStr,
    pub flags: u32,
}

/// Parses renameat2 CLI arguments into an [`Renameat2Config`].
pub fn renameat2_parse<'a>(args: &[&'a CStr]) -> Result<Renameat2Config<'a>, Report<BuiltinError>> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        bail!(BuiltinError::Help);
    }

    let mut olddirfd = None;
    let mut newdirfd = None;
    let mut flags = 0u32;
    let mut oldpath: Option<&'a CStr> = None;
    let mut newpath: Option<&'a CStr> = None;
    let mut i = 0;

    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument("arg"))?;
        i += 1;
        let (key, val) = crate::argparse::split(arg)?;
        match key {
            b"--olddirfd" => {
                olddirfd =
                    crate::argparse::parse_dirfd(crate::argparse::next_val(args, &mut i, val)?)?
            }
            b"--newdirfd" => {
                newdirfd =
                    crate::argparse::parse_dirfd(crate::argparse::next_val(args, &mut i, val)?)?
            }
            b"--flags" => {
                let s = crate::argparse::next_val(args, &mut i, val)?;
                flags = crate::renameat2::parse::flags::parse_rename_flags(s)
                    .change_context(BuiltinError::InvalidArgument("flags"))
                    .attach_opaque(Suggestion(
                        "Use RENAME_NOREPLACE, RENAME_EXCHANGE, RENAME_WHITEOUT, or a hex value (e.g. 0x1)",
                    ))?;
            }
            a if a.starts_with(b"-") => {
                bail!(BuiltinError::InvalidArgument("flag"));
            }
            _ => {
                if oldpath.is_none() {
                    oldpath = Some(arg);
                } else if newpath.is_none() {
                    newpath = Some(arg);
                } else {
                    bail!(BuiltinError::InvalidArgument("arg"));
                }
            }
        }
    }

    let oldpath = oldpath.ok_or(BuiltinError::InvalidArgument("oldpath"))?;
    let newpath = newpath.ok_or(BuiltinError::InvalidArgument("newpath"))?;
    if oldpath.to_bytes().is_empty() {
        bail!(BuiltinError::InvalidArgument("oldpath"));
    }
    if newpath.to_bytes().is_empty() {
        bail!(BuiltinError::InvalidArgument("newpath"));
    }

    Ok(Renameat2Config {
        olddirfd,
        newdirfd,
        oldpath,
        newpath,
        flags,
    })
}
