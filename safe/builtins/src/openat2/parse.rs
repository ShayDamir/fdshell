mod args;
mod flags;

use core::ffi::CStr;
use error_stack::{Report, bail};
use sys::{ImportedFd, openat2::OpenHow};

use crate::error::BuiltinError;

pub struct Openat2Config<'a> {
    pub dirfd: Option<ImportedFd>,
    pub path: &'a CStr,
    pub how: OpenHow,
}

/// Parses openat2 CLI arguments into an [`Openat2Config`].
pub fn openat2_parse<'a>(args: &[&'a CStr]) -> Result<Openat2Config<'a>, Report<BuiltinError>> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        bail!(BuiltinError::Help);
    }

    let mut acc = args::Acc::default();
    let mut i = 0;
    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument("arg"))?;
        i += 1;
        args::parse_arg(&mut acc, args, &mut i, arg)?;
    }

    let path = acc.path.ok_or(BuiltinError::InvalidArgument("path"))?;
    if path.to_bytes().is_empty() {
        bail!(BuiltinError::InvalidArgument("path"));
    }

    Ok(Openat2Config {
        dirfd: acc.dirfd,
        path,
        how: OpenHow {
            flags: acc.open_flags as u64,
            mode: acc.mode,
            resolve: acc.resolve,
        },
    })
}
