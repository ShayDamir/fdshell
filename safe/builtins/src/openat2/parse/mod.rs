mod flags;

use core::ffi::CStr;
use sys::openat2::OpenHow;

pub struct Openat2Config<'a> { pub dirfd: i32, pub path: &'a CStr, pub how: OpenHow }

fn parse_mode(s: &CStr) -> Result<u64, i32> {
    let b = s.to_bytes();
    let (d, r) = if let Some(h) = b.strip_prefix(b"0x") { (h, 16) }
                 else if let Some(o) = b.strip_prefix(b"0o") { (o, 8) }
                 else { (b, 8) };
    u64::from_str_radix(core::str::from_utf8(d).map_err(|_| 22)?, r).map_err(|_| 22)
}

fn parse_dirfd(s: &CStr) -> Result<i32, i32> {
    let b = s.to_bytes();
    if b == b"AT_FDCWD" { Ok(-100) } else {
        let s = core::str::from_utf8(b).map_err(|_| 22)?;
        s.parse().map_err(|_| 22)
    }
}

/// Parses openat2 CLI arguments into an [`Openat2Config`].
///
/// Returns:
/// - `Err(0)` — `--help` or `-h` was passed
/// - `Err(22)` — bad flag name, missing value, etc.
/// - `Err(2)` — empty path
///
/// # Example
///
/// ```rust
/// use core::ffi::CStr;
/// use std::ffi::CString;
///
/// let a = CString::new("--flags").unwrap();
/// let b = CString::new("O_RDONLY").unwrap();
/// let c = CString::new("package.nix").unwrap();
/// let args = [a.as_c_str(), b.as_c_str(), c.as_c_str()];
/// let cfg = builtins::openat2::parse::openat2_parse(&args);
/// match cfg {
///     Ok(cfg) => {
///         assert_eq!(cfg.dirfd, -100);
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
/// let a = CString::new("--bad").unwrap();
/// let b = CString::new("x").unwrap();
/// let args = [a.as_c_str(), b.as_c_str()];
/// match builtins::openat2::parse::openat2_parse(&args) {
///     Err(22) => {}
///     _ => panic!("expected Err(22)"),
/// }
/// ```
pub fn openat2_parse<'a>(args: &[&'a CStr]) -> Result<Openat2Config<'a>, i32> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        return Err(0);
    }

    let mut dirfd = -100;
    let mut open_flags = 0;
    let mut mode: u64 = 0;
    let mut resolve: u64 = 0;
    let mut path: Option<&'a CStr> = None;
    let mut i = 0;

    while i < args.len() {
        let arg = args.get(i).ok_or(22)?;
        i += 1;
        let (key, val) = crate::argparse::split(arg)?;
        match key {
            b"--dirfd" => dirfd = parse_dirfd(crate::argparse::next_val(args, &mut i, val)?)?,
            b"--flags" => open_flags = flags::parse_open_flags(crate::argparse::next_val(args, &mut i, val)?)?,
            b"--mode" => mode = parse_mode(crate::argparse::next_val(args, &mut i, val)?)?,
            b"--resolve" => resolve = flags::parse_resolve_flags(crate::argparse::next_val(args, &mut i, val)?)?,
            a if a.starts_with(b"-") => return Err(22),
            _ => {
                if path.is_some() { return Err(22); }
                path = Some(arg);
            }
        }
    }

    let path = path.ok_or(22)?;
    if path.to_bytes().is_empty() { return Err(2); }

    Ok(Openat2Config { dirfd, path, how: OpenHow { flags: open_flags as u64, mode, resolve } })
}
