mod arms;

use super::{Redirect, RedirectDef};
use crate::error::redirect::OpenRedirectError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::Report;
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub fn resolve_redirects(
    redirects: &[RedirectDef],
    opened: &[LocalFd],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<Redirect>, Report<OpenRedirectError>> {
    let mut opened_iter = opened.iter();
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let mut resolved = Vec::new();
    // Sources must be cloned above every target of this command, not just the
    // one they apply to: a later dup2 can otherwise clobber and then close an
    // earlier redirect's target fd.
    let min_fd = RedirectDef::max_target(redirects)
        .checked_add(1)
        .ok_or(OpenRedirectError::FdNumberOutOfRange)?;
    for r in redirects {
        resolved.push(resolve_one(r, &mut opened_iter, &mut cache, min_fd, cell)?);
    }
    Ok(resolved)
}

fn resolve_one(
    r: &RedirectDef,
    opened_iter: &mut core::slice::Iter<'_, LocalFd>,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    min_fd: i32,
    cell: &ForkCell<ShellState>,
) -> Result<Redirect, Report<OpenRedirectError>> {
    match &r.source {
        super::RedirectSource::Var(var) => arms::resolve_var(var, r.export_to, min_fd, cell),
        super::RedirectSource::Path(_) => arms::resolve_path(r.export_to, min_fd, opened_iter),
        super::RedirectSource::HereString(word) => Ok(Redirect::Dup {
            export_to: r.export_to,
            local: super::herestring::here_string(word, cache, cell)?,
        }),
        super::RedirectSource::Dup(from) => arms::resolve_dup(r.export_to, min_fd, *from),
        super::RedirectSource::Close => Ok(Redirect::Close {
            export_to: r.export_to,
        }),
    }
}

#[cfg(test)]
mod tests;
