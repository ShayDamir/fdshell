mod strict;

use super::{RedirectDef, RedirectDirection, RedirectSource};
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
    // `glob::matches` re-borrows the cell per redirect target (RefCell,
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
    let noclobber_flag = noclobber && r.write_flag();
    let opened = process_cwd_open(r, path, mask, noclobber_flag, cell)?;
    // Re-home the freshly opened fd above every redirect target of this
    // command: a `dup2` of another redirect must never clobber the open
    // fd and then have this descriptor's drop close the target.
    let rehomed = opened
        .try_clone_above(min_fd)
        .change_context(OpenRedirectError::Open)?;
    drop(opened);
    Ok(rehomed)
}

/// Open a redirect target through `dir`: strict (a base dirfd is present)
/// opens via `openat2` on the pinned `%CWD` fd; non-strict (no `dir`) keeps
/// the plain process-CWD `open()` call. `O_CLOEXEC` is forced by `sys`.
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

/// The non-strict path, byte-for-byte as before: process-CWD `open()`.
fn process_cwd_open(
    r: &RedirectDef,
    path: &ShortCStr,
    mask: &[bool],
    noclobber_flag: bool,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    let target = redirect_target(path, mask, cell)?;
    let name = target.export();
    if noclobber_flag {
        return open_noclobber(&name, &target);
    }
    sys::openat2::open(&name, r.direction.open_flags()).change_context(OpenRedirectError::Open)
}

/// Glob a redirect target: `> 1` match is ambiguous, `0` matches open the
/// literal word (bash passes unmatched redirect patterns through; `failglob`
/// does not apply to redirects), `1` match opens that match.
///
/// The target word is folded first (POSIX #4.1): bash folds every unquoted
/// escape pair at tokenization, and a redirect target is never substituted,
/// so `> a\*b` opens the file named `a*b`.
fn redirect_target(
    path: &ShortCStr,
    mask: &[bool],
    cell: &ForkCell<ShellState>,
) -> Result<ShortCStr, Report<OpenRedirectError>> {
    let (target, target_mask) =
        crate::bytes::fold::fold_word(path, mask).change_context(OpenRedirectError::Never)?;
    let matches = crate::glob::matches(&target, &target_mask, cell, None)
        .change_context(OpenRedirectError::Glob)?;
    match matches.len() {
        0 => Ok(target),
        1 => matches
            .first()
            .cloned()
            .ok_or(OpenRedirectError::Never.into()),
        _ => Err(Report::new(OpenRedirectError::AmbiguousRedirect {
            name: path.clone(),
        })),
    }
}

/// Open a fresh file exclusively: `EEXIST` means the target exists (noclobber).
fn open_noclobber(
    name: &ExportedCStr,
    path: &ShortCStr,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    match sys::openat2::open(name, RedirectDirection::noclobber_flags()) {
        Ok(fd) => Ok(fd),
        Err(e) if e.errno() == sys::errno::EEXIST => {
            Err(OpenRedirectError::Noclobber { name: path.clone() }.into())
        }
        Err(e) => Err(Report::new(OpenRedirectError::Open).attach(e)),
    }
}

#[cfg(test)]
mod tests;
