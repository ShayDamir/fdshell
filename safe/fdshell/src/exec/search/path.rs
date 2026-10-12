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

/// The openable pathname built from one PATH entry: strict bans an absolute
/// entry (root-escape ban) and opens a relative one against the pinned
/// `%CWD` base fd; non-strict joins it verbatim (behavior unchanged).
fn search_entry(
    dir: &ShortCStr,
    bin: &ShortCStr,
    strict: bool,
    cwd: Option<&LocalFd>,
) -> Result<ShortCStr, Report<ChildProcessError>> {
    if strict && dir.starts_with(b"/") {
        bail!(ChildProcessError::StrictPath(dir.clone()));
    }
    let pathname = join(dir, bin);
    Ok(open_checked(&pathname, strict, cwd)
        .is_ok()
        .then(|| pathname.clone())
        .ok_or(ChildProcessError::NotFound(pathname.clone()))?)
}

/// Search `PATH` for `bin`: the first entry that yields an openable pathname
/// (a `:`-separated empty entry means `.`, bash). Strict bans an absolute
/// `bin` outright and absolute entries inside the loop — before the entry is
/// probed, so a root-escaping `PATH` fails loudly instead of half-working.
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
    for dir in path_str.split(b':') {
        if dir.is_empty() {
            continue;
        }
        if let Ok(pathname) = search_entry(&dir, bin, strict, cwd) {
            return Ok(pathname);
        }
    }
    bail!(ChildProcessError::NotFound(bin.clone()))
}
