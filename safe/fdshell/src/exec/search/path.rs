//! The PATH search: the pathname a bare `bin` resolves to.

use error_stack::{Report, bail};
use sys::{LocalFd, ShortCStr};

use crate::error::child_process::ChildProcessError;

use super::open_checked;

/// `dir + "/" + bin`, or `bin` itself when `dir` is `.` (the relative base
/// *is* the working directory — no `./` prefix in a pinned/displayed path).
fn join(dir: &ShortCStr, bin: &ShortCStr) -> ShortCStr {
    if dir.eq_bytes(b".") {
        return bin.clone();
    }
    let slash: ShortCStr = c"/".into();
    ShortCStr::concat(&[dir, &slash, bin])
}

/// One `PATH` entry as a search base: an empty entry is `.` (bash semantics)
/// under strict; non-strict keeps skipping it (the pre-strict behavior).
pub(super) fn entry(dir: ShortCStr, strict: bool) -> Option<ShortCStr> {
    match (dir.is_empty(), strict) {
        (true, true) => Some(c".".into()),
        (true, false) => None,
        (false, _) => Some(dir),
    }
}

/// The openable pathname built from one PATH entry, opened against the
/// strict `%CWD` base when `strict` (non-strict: the process CWD).
fn search_entry(
    dir: &ShortCStr,
    bin: &ShortCStr,
    strict: bool,
    cwd: Option<&LocalFd>,
) -> Result<ShortCStr, Report<ChildProcessError>> {
    let pathname = join(dir, bin);
    open_checked(&pathname, strict, cwd)?;
    Ok(pathname)
}

/// Search `PATH` for `bin`: the first entry that yields an openable pathname.
/// Strict bans an absolute `bin` outright and bans an absolute entry the
/// moment the loop reaches it — a root-escaping `PATH` fails loudly instead of
/// half-working.
pub(super) fn search_path_str(
    bin: &ShortCStr,
    strict: bool,
    cwd: Option<&LocalFd>,
) -> Result<ShortCStr, Report<ChildProcessError>> {
    // An empty name resolves to no file: `dir/` is openable, so without this
    // the PATH search would "find" a directory and fail later in execveat.
    if bin.is_empty() {
        bail!(ChildProcessError::NotFound(bin.clone()));
    }
    if strict && bin.starts_with(b"/") {
        bail!(ChildProcessError::StrictPath(bin.clone()));
    }
    let path_str = sys::env::getenv(c"PATH").unwrap_or(c"/usr/local/bin:/usr/bin:/bin".into());
    for dir in path_str.split(b':').filter_map(|dir| entry(dir, strict)) {
        if strict && dir.starts_with(b"/") {
            bail!(ChildProcessError::StrictPath(dir));
        }
        if let Ok(pathname) = search_entry(&dir, bin, strict, cwd) {
            return Ok(pathname);
        }
    }
    bail!(ChildProcessError::NotFound(bin.clone()))
}
