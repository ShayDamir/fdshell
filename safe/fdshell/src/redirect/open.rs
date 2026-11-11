// mutants: -C --lib -C --test -C strict -- -E 'test(/^redirect::open::/) | binary(strict)'

mod cwd;
mod noclobber;
mod strict;
mod target;

use noclobber::Decision;

use super::{RedirectDef, RedirectSource};
use crate::error::redirect::OpenRedirectError;
use crate::redirect::OpenedTarget;
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
) -> Result<Vec<OpenedTarget>, Report<OpenRedirectError>> {
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
) -> Result<OpenedTarget, Report<OpenRedirectError>> {
    let noclobber = noclobber && r.write_flag();
    // The strict arm needs no magic decision: the absolute ban runs on the
    // word (fast path) or every expanded field (substitution path) **before**
    // the walk, so a magic name — always absolute — can never exist here.
    if let Some(base) = base {
        let opened = strict::target(r, source, cache, base, min_fd, noclobber, cell)?;
        return Ok(OpenedTarget::new(Some(opened), None));
    }
    let name = target::resolve(source, cache, None, cell)?;
    let magic = target::magic(&name);
    let decision = if noclobber {
        cwd::gate(&name)
    } else {
        Decision::Plain
    };
    // A magic word's plain-arm open belongs to the resolver, run at its own
    // point in the walk against the fd state the preceding defs leave
    // (`redirect/resolve/fd_path.rs`): the pass flags the entry and opens
    // nothing (task #482). Only the gate's own arms stay here — the refusal,
    // and the exclusive create whose errno *is* the decision (task #480).
    if magic.is_some() && matches!(decision, Decision::Plain) {
        return Ok(OpenedTarget::new(None, magic));
    }
    let opened = cwd::open(r, &name, decision)?;
    // Re-home the freshly opened fd above every redirect target of this
    // command: a `dup2` of another redirect must never clobber the open
    // fd and then have this descriptor's drop close the target.
    let rehomed = opened
        .try_clone_above(min_fd)
        .change_context(OpenRedirectError::Open)?;
    drop(opened);
    Ok(OpenedTarget::new(Some(rehomed), magic))
}

#[cfg(test)]
mod tests;
