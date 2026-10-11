mod args;
mod flags;

use core::ffi::CStr;
use error_stack::{Report, bail};
use sys::fcntl::O_CREAT;
use sys::{ImportedFd, openat2::OpenHow};

use crate::error::BuiltinError;

pub struct Openat2Config<'a> {
    pub dirfd: Option<ImportedFd>,
    /// If set, the opened fd must be the same `(dev, ino)` as this reference.
    pub same_as: Option<ImportedFd>,
    pub path: &'a CStr,
    pub how: OpenHow,
}

/// Parses openat2 CLI arguments into an [`Openat2Config`].
pub fn openat2_parse<'a>(
    args: &[&'a CStr],
    strict: bool,
) -> Result<Openat2Config<'a>, Report<BuiltinError>> {
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

    crate::strict::require_dirfd(strict, acc.dirfd.as_ref())?;
    crate::strict::require_relative(strict, path)?;

    // Without `--mode`, only `O_CREAT` has a mode: it gets the POSIX `open(2)`
    // default (`sys::openat2::open` passes the same), which the kernel masks
    // with the umask. An explicit `--mode 0` stays 0; without `O_CREAT` the
    // kernel ignores the mode, so it stays 0.
    let creating = acc.open_flags & O_CREAT != 0;
    let mode = if acc.mode_given || !creating {
        acc.mode
    } else {
        u64::from(crate::mode::CREATE_MODE)
    };

    Ok(Openat2Config {
        dirfd: acc.dirfd,
        same_as: acc.same_as,
        path,
        how: OpenHow {
            flags: acc.open_flags as u64,
            mode,
            resolve: acc.resolve,
        },
    })
}
