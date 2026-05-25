mod flags;

use core::ffi::CStr;
use sys::DupFd;
use sys::errno::{EINVAL, ENOENT};

pub struct Renameat2Config<'a> {
    pub olddirfd: Option<DupFd>,
    pub newdirfd: Option<DupFd>,
    pub oldpath: &'a CStr,
    pub newpath: &'a CStr,
    pub flags: u32,
}

/// Parses renameat2 CLI arguments into an [`Renameat2Config`].
///
/// Returns:
/// - `Err(sys::errno::HELP)` — `--help` or `-h` was passed
/// - `Err(sys::errno::EINVAL)` — bad flag name, missing value, etc.
/// - `Err(sys::errno::ENOENT)` — empty path
///
/// # Example
///
/// ```rust
/// use std::ffi::CString;
///
/// let a = CString::from(c"--flags");
/// let b = CString::from(c"RENAME_NOREPLACE");
/// let c = CString::from(c"old.txt");
/// let d = CString::from(c"new.txt");
/// let args = [a.as_c_str(), b.as_c_str(), c.as_c_str(), d.as_c_str()];
/// let cfg = builtins::renameat2::parse::renameat2_parse(&args);
/// match cfg {
///     Ok(cfg) => {
///         assert!(cfg.olddirfd.is_none());
///         assert!(cfg.newdirfd.is_none());
///         assert_eq!(cfg.flags, sys::renameat2::RENAME_NOREPLACE);
///         assert_eq!(cfg.oldpath.to_bytes(), b"old.txt");
///         assert_eq!(cfg.newpath.to_bytes(), b"new.txt");
///     }
///     _ => panic!("expected Ok"),
/// }
/// ```
pub fn renameat2_parse<'a>(args: &[&'a CStr]) -> Result<Renameat2Config<'a>, i32> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        return Err(sys::errno::HELP);
    }

    let mut olddirfd = None;
    let mut newdirfd = None;
    let mut flags = 0u32;
    let mut oldpath: Option<&'a CStr> = None;
    let mut newpath: Option<&'a CStr> = None;
    let mut i = 0;

    while i < args.len() {
        let arg = args.get(i).ok_or(EINVAL)?;
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
                flags = flags::parse_rename_flags(crate::argparse::next_val(args, &mut i, val)?)?
            }
            a if a.starts_with(b"-") => return Err(EINVAL),
            _ => {
                if oldpath.is_none() {
                    oldpath = Some(arg);
                } else if newpath.is_none() {
                    newpath = Some(arg);
                } else {
                    return Err(EINVAL);
                }
            }
        }
    }

    let oldpath = oldpath.ok_or(EINVAL)?;
    let newpath = newpath.ok_or(EINVAL)?;
    if oldpath.to_bytes().is_empty() {
        return Err(ENOENT);
    }
    if newpath.to_bytes().is_empty() {
        return Err(ENOENT);
    }

    Ok(Renameat2Config {
        olddirfd,
        newdirfd,
        oldpath,
        newpath,
        flags,
    })
}
