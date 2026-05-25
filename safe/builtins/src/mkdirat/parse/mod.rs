use core::ffi::CStr;
use sys::errno::{EINVAL, ENOENT};
use sys::DupFd;

pub struct MkdiratConfig<'a> {
    pub dirfd: Option<DupFd>,
    pub path: &'a CStr,
    pub mode: u32,
    pub resolve: u64,
}

/// Parses mkdirat CLI arguments into an [`MkdiratConfig`].
///
/// Returns:
/// - `Err(sys::errno::HELP)` — `--help` or `-h` was passed
/// - `Err(sys::errno::EINVAL)` — bad flag name, missing value, etc.
///
/// # Example
///
/// ```rust
/// use std::ffi::CString;
///
/// let a = CString::from(c"--mode");
/// let b = CString::from(c"755");
/// let c = CString::from(c"newdir");
/// let args = [a.as_c_str(), b.as_c_str(), c.as_c_str()];
/// let cfg = builtins::mkdirat::parse::mkdirat_parse(&args);
/// match cfg {
///     Ok(cfg) => {
///         assert!(cfg.dirfd.is_none());
///         assert_eq!(cfg.mode, 0o755);
///         assert_eq!(cfg.path.to_bytes(), b"newdir");
///     }
///     _ => panic!("expected Ok"),
/// }
/// ```
pub fn mkdirat_parse<'a>(args: &[&'a CStr]) -> Result<MkdiratConfig<'a>, i32> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        return Err(sys::errno::HELP);
    }

    let mut dirfd = None;
    let mut mode: u32 = 0;
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
            b"--mode" => {
                mode = crate::argparse::parse_mode(crate::argparse::next_val(args, &mut i, val)?)?
                    as u32
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

    Ok(MkdiratConfig {
        dirfd,
        path,
        mode,
        resolve,
    })
}
