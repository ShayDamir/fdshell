use error_stack::{Report, ResultExt, bail};
use sys::fcntl::O_PATH;
use sys::{LocalFd, ShortCStr};

use crate::error::child_process::ChildProcessError;

fn search_path_str(bin: &ShortCStr) -> Result<ShortCStr, Report<ChildProcessError>> {
    let path_str = sys::env::getenv(c"PATH").unwrap_or(c"/usr/local/bin:/usr/bin:/bin".into());
    let slash: ShortCStr = c"/".into();
    for dir in path_str.split(b':') {
        if dir.is_empty() {
            continue;
        }
        let pathname = ShortCStr::concat(&[&dir, &slash, bin]);
        if sys::openat2::open(pathname.export(), O_PATH).is_ok() {
            return Ok(pathname);
        }
    }
    bail!(ChildProcessError::NotFound(bin.clone()))
}

fn open_path_str(pathname: &ShortCStr) -> Result<LocalFd, Report<ChildProcessError>> {
    sys::openat2::open(pathname.export(), O_PATH)
        .change_context_lazy(|| ChildProcessError::NotFound(pathname.clone()))
}

pub fn resolve_path(bin: &ShortCStr) -> Result<LocalFd, Report<ChildProcessError>> {
    if bin.contains(b'/') {
        open_path_str(bin)
    } else {
        open_path_str(&search_path_str(bin)?)
    }
}

/// The path `resolve_path` would exec, for display (e.g. `type`).
pub fn resolve_path_str(bin: &ShortCStr) -> Result<ShortCStr, Report<ChildProcessError>> {
    if bin.contains(b'/') {
        open_path_str(bin)?;
        Ok(bin.clone())
    } else {
        search_path_str(bin)
    }
}
