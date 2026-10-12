mod cwd;
mod strict;
mod target;

use super::{RedirectDef, RedirectSource};
use crate::error::redirect::OpenRedirectError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::AtFd;
use sys::ExportedCStr;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// The pinned `%CWD` dirfd clone strict mode opens every target through;
/// `None` for non-strict (the process-CWD `open()` calls, unchanged) and for
/// the defensive case of an unseeded `%CWD` (startup is the only producer).
fn strict_base(
    cell: &ForkCell<ShellState>,
    strict: bool,
) -> Result<Option<LocalFd>, Report<OpenRedirectError>> {
    if !strict {
        return Ok(None);
    }
    let state = cell.borrow().change_context(OpenRedirectError::Never)?;
    state
        .cwd_fd()
        .map(|fd| fd.try_clone().change_context(OpenRedirectError::Open))
        .transpose()
}

pub fn open_redirect_files(
    redirects: &[RedirectDef],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<LocalFd>, Report<OpenRedirectError>> {
    // The options and the pinned `%CWD` fd are read in block-scoped borrows:
    // the target glob re-borrows the cell per redirect target (RefCell,
    // LESSONS.md). Strict opens every target through `%CWD`.
    let strict = {
        let state = cell.borrow().change_context(OpenRedirectError::Never)?;
        state.strict()
    };
    let base = strict_base(cell, strict)?;
    let noclobber = {
        let state = cell.borrow().change_context(OpenRedirectError::Never)?;
        state.options & crate::options::NOCLOBBER != 0
    };
    let min_fd = RedirectDef::max_target(redirects)
        .checked_add(1)
        .ok_or(OpenRedirectError::FdNumberOutOfRange)?;
    let mut fds = Vec::new();
    for r in redirects {
        if let RedirectSource::Path { path, mask } = &r.source {
            let opened = open_redirect(r, path, mask, noclobber, base.as_ref(), min_fd, cell)?;
            fds.push(opened);
        }
    }
    Ok(fds)
}

fn open_redirect(
    r: &RedirectDef,
    path: &ShortCStr,
    mask: &[bool],
    noclobber: bool,
    base: Option<&LocalFd>,
    min_fd: i32,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    if let Some(base) = base {
        return strict::target(r, base, min_fd, noclobber, cell);
    }
    let opened = cwd::open(r, path, mask, noclobber && r.write_flag(), cell)?;
    // Re-home the freshly opened fd above every redirect target of this
    // command: a `dup2` of another redirect must never clobber the open
    // fd and then have this descriptor's drop close the target.
    let rehomed = opened
        .try_clone_above(min_fd)
        .change_context(OpenRedirectError::Open)?;
    drop(opened);
    Ok(rehomed)
}

/// The `open_target` form of a redirect open: with a dirfd (`Some`) it is
/// `openat2` on the pinned `%CWD` fd — strict mode's capability route;
/// without one (`None`) it is the plain process-CWD `open()`. `O_CLOEXEC` is
/// forced by `sys`.
pub(super) fn open_target(
    name: &ExportedCStr,
    how: &sys::openat2::OpenHow,
    dir: Option<&LocalFd>,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    match dir {
        Some(fd) => sys::openat2::openat2(AtFd::from(fd), name, how),
        None => sys::openat2::open(name, sys::openat2::flags_of(how)),
    }
    .change_context(OpenRedirectError::Open)
}

#[cfg(test)]
mod tests;
