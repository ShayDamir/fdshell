use super::{Redirect, RedirectDef};
use crate::error::redirect::OpenRedirectError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
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
    for r in redirects {
        resolved.push(resolve_one(r, &mut opened_iter, &mut cache, cell)?);
    }
    Ok(resolved)
}

fn resolve_one(
    r: &RedirectDef,
    opened_iter: &mut core::slice::Iter<'_, LocalFd>,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<Redirect, Report<OpenRedirectError>> {
    let min_fd = r
        .export_to
        .checked_add(1)
        .ok_or(OpenRedirectError::FdNumberOutOfRange)?;
    match &r.source {
        super::RedirectSource::Var(var) => {
            let state = cell.borrow().change_context(OpenRedirectError::Never)?;
            let local = state
                .fds
                .get(var)
                .ok_or_else(|| OpenRedirectError::VarNotFound { var: var.clone() })?
                .fd
                .try_clone_above(min_fd)
                .change_context(OpenRedirectError::Open)?;
            Ok(Redirect::Dup {
                export_to: r.export_to,
                local,
            })
        }
        super::RedirectSource::Path(_) => {
            let local = opened_iter
                .next()
                .ok_or(OpenRedirectError::Open)?
                .try_clone()
                .change_context(OpenRedirectError::Open)?;
            Ok(Redirect::Dup {
                export_to: r.export_to,
                local,
            })
        }
        super::RedirectSource::HereString(word) => Ok(Redirect::Dup {
            export_to: r.export_to,
            local: super::herestring::here_string(word, cache, cell)?,
        }),
        super::RedirectSource::Dup(from) => {
            let imported = sys::ImportedFd::from_number(*from)
                .change_context_lazy(|| OpenRedirectError::FdNotOpen { n: *from })?;
            let local = imported
                .try_dup()
                .change_context_lazy(|| OpenRedirectError::DupFdFailed { n: *from })?;
            Ok(Redirect::Dup {
                export_to: r.export_to,
                local,
            })
        }
        super::RedirectSource::Close => Ok(Redirect::Close {
            export_to: r.export_to,
        }),
    }
}

#[cfg(test)]
mod tests;
