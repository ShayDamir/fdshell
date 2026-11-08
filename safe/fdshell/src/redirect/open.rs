// mutants: -C --lib -- -E 'test(/^redirect::open::/)'

mod cwd;
mod strict;
mod target;

use super::{RedirectDef, RedirectSource};
use crate::error::redirect::OpenRedirectError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

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
    let base = strict::strict_base(cell, strict)?;
    let noclobber = {
        let state = cell.borrow().change_context(OpenRedirectError::Never)?;
        state.options & crate::options::NOCLOBBER != 0
    };
    let min_fd = RedirectDef::max_target(redirects)
        .checked_add(1)
        .ok_or(OpenRedirectError::FdNumberOutOfRange)?;
    let mut fds = Vec::new();
    // One `%fd` cache per open call: the target word runs through the
    // substitution pipeline, whose `%name` arm looks fds up in a cache exactly
    // as the argument pipeline does (`substitute/words.rs`).
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    for r in redirects {
        if let RedirectSource::Path { .. } = &r.source {
            let opened = open_redirect(
                r,
                &r.source,
                &mut cache,
                noclobber,
                base.as_ref(),
                min_fd,
                cell,
            )?;
            fds.push(opened);
        }
    }
    Ok(fds)
}

fn open_redirect(
    r: &RedirectDef,
    source: &RedirectSource,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    noclobber: bool,
    base: Option<&LocalFd>,
    min_fd: i32,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    if let Some(base) = base {
        return strict::target(r, source, cache, base, min_fd, noclobber, cell);
    }
    let opened = cwd::open(r, source, cache, noclobber && r.write_flag(), cell)?;
    // Re-home the freshly opened fd above every redirect target of this
    // command: a `dup2` of another redirect must never clobber the open
    // fd and then have this descriptor's drop close the target.
    let rehomed = opened
        .try_clone_above(min_fd)
        .change_context(OpenRedirectError::Open)?;
    drop(opened);
    Ok(rehomed)
}

#[cfg(test)]
mod tests;
