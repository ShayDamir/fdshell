use error_stack::{Report, ResultExt, bail};
use hashbrown::HashMap;
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

/// The path `bin` would exec, honoring the hash table: a bare name with a
/// table entry opens the pinned path; a pin that no longer exists falls back
/// to the PATH search (self-heal). Names containing `/` bypass the table.
pub fn resolve_path_str(
    bin: &ShortCStr,
    table: &HashMap<ShortCStr, ShortCStr>,
) -> Result<ShortCStr, Report<ChildProcessError>> {
    if bin.contains(b'/') {
        open_path_str(bin)?;
        return Ok(bin.clone());
    }
    if let Some(pinned) = table.get(bin)
        && sys::openat2::open(pinned.export(), O_PATH).is_ok()
    {
        return Ok(pinned.clone());
    }
    search_path_str(bin)
}

pub fn resolve_path(
    bin: &ShortCStr,
    table: &HashMap<ShortCStr, ShortCStr>,
) -> Result<LocalFd, Report<ChildProcessError>> {
    open_path_str(&resolve_path_str(bin, table)?)
}
