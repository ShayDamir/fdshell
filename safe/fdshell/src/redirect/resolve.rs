use super::{Redirect, RedirectDef};
use crate::error::redirect::OpenRedirectError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fcntl::SEEK_SET;
use sys::fork_cell::ForkCell;

pub fn resolve_redirects(
    redirects: &[RedirectDef],
    opened: &[LocalFd],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<Redirect>, Report<OpenRedirectError>> {
    let mut opened_iter = opened.iter();
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let mut resolved = Vec::new();
    for r in redirects {
        let local = resolve_one(r, &mut opened_iter, &mut cache, cell)?;
        resolved.push(r.resolve(local));
    }
    Ok(resolved)
}

fn resolve_one(
    r: &RedirectDef,
    opened_iter: &mut core::slice::Iter<'_, LocalFd>,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    match &r.source {
        super::RedirectSource::Var(var) => {
            let min_fd = r
                .export_to
                .checked_add(1)
                .ok_or(OpenRedirectError::FdNumberOutOfRange)?;
            let state = cell.borrow().change_context(OpenRedirectError::Never)?;
            state
                .fds
                .get(var)
                .ok_or_else(|| OpenRedirectError::VarNotFound { var: var.clone() })?
                .fd
                .try_clone_above(min_fd)
                .change_context(OpenRedirectError::Open)
        }
        super::RedirectSource::Path(_) => opened_iter
            .next()
            .ok_or(OpenRedirectError::Open)?
            .try_clone()
            .change_context(OpenRedirectError::Open),
        super::RedirectSource::HereString(word) => here_string(word, cache, cell),
    }
}

/// Expand the word and back it with a seeked-to-zero memfd as the new stdin.
fn here_string(
    word: &ShortCStr,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    let data = crate::substitute::substitute_arg(word, cache, cell)
        .change_context(OpenRedirectError::HereStringExpand)?;
    let fd = sys::memfd::memfd_create().change_context(OpenRedirectError::HereStringCreate)?;
    let mut payload = Vec::with_capacity(data.len() + 1);
    payload.extend_from_slice(data.as_bytes().change_context(OpenRedirectError::Never)?);
    payload.push(b'\n');
    sys::rw::write_all(&fd, &payload).change_context(OpenRedirectError::HereStringCreate)?;
    sys::rw::lseek(&fd, 0, SEEK_SET).change_context(OpenRedirectError::HereStringCreate)?;
    Ok(fd)
}

#[cfg(test)]
mod tests;
