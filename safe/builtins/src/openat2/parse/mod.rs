mod flags;

use core::ffi::CStr;
use sys::ImportedFd;
use sys::errno::{EINVAL, ENOENT};
use sys::openat2::OpenHow;

pub struct Openat2Config<'a> {
    pub dirfd: Option<ImportedFd>,
    pub path: &'a CStr,
    pub how: OpenHow,
}

/// Parses openat2 CLI arguments into an [`Openat2Config`].
///
/// Returns:
/// - `Err(sys::errno::HELP)` — `--help` or `-h` was passed
/// - `Err(sys::errno::EINVAL)` — bad flag name, missing value, etc.
/// - `Err(sys::errno::ENOENT)` — empty path
///
/// # Example
///
/// ```rust
/// use core::ffi::CStr;
/// use std::ffi::CString;
///
/// let a = CString::from(c"--flags");
/// let b = CString::from(c"O_RDONLY");
/// let c = CString::from(c"package.nix");
/// let args = [a.as_c_str(), b.as_c_str(), c.as_c_str()];
/// let cfg = builtins::openat2::parse::openat2_parse(&args);
/// match cfg {
///     Ok(cfg) => {
///         assert!(cfg.dirfd.is_none());
///         assert_eq!(cfg.path.to_bytes(), b"package.nix");
///         assert_eq!(cfg.how.flags, 0);
///     }
///     _ => panic!("expected Ok"),
/// }
/// ```
///
/// # Error example
///
/// ```rust
/// use std::ffi::CString;
///
/// let a = CString::from(c"--bad");
/// let b = CString::from(c"x");
/// let args = [a.as_c_str(), b.as_c_str()];
/// match builtins::openat2::parse::openat2_parse(&args) {
///     Err(sys::errno::EINVAL) => {}
///     _ => panic!("expected Err(sys::errno::EINVAL)"),
/// }
/// ```
pub fn openat2_parse<'a>(args: &[&'a CStr]) -> Result<Openat2Config<'a>, i32> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        return Err(sys::errno::HELP);
    }

    let mut dirfd = None;
    let mut open_flags = 0;
    let mut mode: u64 = 0;
    let mut resolve: u64 = 0;
    let mut path: Option<&'a CStr> = None;
    let mut i = 0;

    while i < args.len() {
        let arg = args.get(i).ok_or(EINVAL)?;
        i += 1;
        let (key, val) = crate::argparse::split(arg)?;
        match key {
            b"--dirfd" => {
                dirfd = crate::argparse::parse_dirfd(crate::argparse::next_val(args, &mut i, val)?)?
            }
            b"--flags" => {
                open_flags = flags::parse_open_flags(crate::argparse::next_val(args, &mut i, val)?)?
            }
            b"--mode" => {
                mode = crate::argparse::parse_mode(crate::argparse::next_val(args, &mut i, val)?)?
            }
            b"--resolve" => {
                resolve = crate::resolve::parse_resolve_flags(crate::argparse::next_val(
                    args, &mut i, val,
                )?)?
            }
            a if a.starts_with(b"-") => return Err(EINVAL),
            _ => {
                if path.is_some() {
                    return Err(EINVAL);
                }
                path = Some(arg);
            }
        }
    }

    let path = path.ok_or(EINVAL)?;
    if path.to_bytes().is_empty() {
        return Err(ENOENT);
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
