mod path;

use error_stack::{Report, ResultExt, bail};
use hashbrown::HashMap;
use sys::fcntl::O_PATH;
use sys::openat2::OpenHow;
use sys::{AtFd, LocalFd, ShortCStr};

use crate::error::child_process::ChildProcessError;

use path::search_path_str;

/// Resolve `path` (O_PATH), banning an absolute pathname while strict is on.
pub(super) fn open_checked(
    pathname: &ShortCStr,
    strict: bool,
    cwd: Option<&LocalFd>,
) -> Result<LocalFd, Report<ChildProcessError>> {
    if strict && pathname.starts_with(b"/") {
        bail!(ChildProcessError::StrictPath(pathname.clone()));
    }
    sys::openat2::openat2(
        base(strict, cwd),
        pathname.export(),
        &OpenHow::new(O_PATH as u64, 0),
    )
    .change_context_lazy(|| ChildProcessError::NotFound(pathname.clone()))
}

/// The PATH-search base: the shell's pinned `%CWD` fd under strict (capability
/// base, follows `cd`), the process CWD otherwise (non-strict is unchanged).
fn base(strict: bool, cwd: Option<&LocalFd>) -> AtFd<'_> {
    if strict && let Some(fd) = cwd {
        return AtFd::from(fd);
    }
    AtFd::cwd()
}

/// The pinned table entry, if strict allows it and it opens. A pin that no
/// longer exists returns `None` and the caller falls back to the PATH search
/// (self-heal).
fn pinned_path(
    bin: &ShortCStr,
    table: &HashMap<ShortCStr, ShortCStr>,
    strict: bool,
    cwd: Option<&LocalFd>,
) -> Result<Option<ShortCStr>, Report<ChildProcessError>> {
    let Some(pinned) = table.get(bin).cloned() else {
        return Ok(None);
    };
    if strict && pinned.starts_with(b"/") {
        bail!(ChildProcessError::StrictPath(pinned));
    }
    Ok(open_checked(&pinned, strict, cwd)
        .is_ok()
        .then(|| pinned.clone()))
}

/// The path `bin` would exec, honoring the hash table: a bare name with a
/// table entry opens the pinned path; a pin that no longer exists falls back
/// to the PATH search (self-heal). Names containing `/` bypass the table.
/// Strict bans an absolute program word, an absolute pinned path and an
/// absolute PATH entry; relative lookups resolve against the `cwd` base fd.
pub fn resolve_path_str(
    bin: &ShortCStr,
    table: &HashMap<ShortCStr, ShortCStr>,
    strict: bool,
    cwd: Option<&LocalFd>,
) -> Result<ShortCStr, Report<ChildProcessError>> {
    if bin.contains(b'/') {
        open_checked(bin, strict, cwd)?;
        return Ok(bin.clone());
    }
    if let Some(pinned) = pinned_path(bin, table, strict, cwd)? {
        return Ok(pinned);
    }
    search_path_str(bin, strict, cwd)
}

pub fn resolve_path(
    bin: &ShortCStr,
    table: &HashMap<ShortCStr, ShortCStr>,
    strict: bool,
    cwd: Option<&LocalFd>,
) -> Result<LocalFd, Report<ChildProcessError>> {
    open_checked(&resolve_path_str(bin, table, strict, cwd)?, strict, cwd)
}

#[cfg(test)]
mod tests;
