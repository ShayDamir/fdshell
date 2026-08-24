use crate::error::redirect::OpenRedirectError;
use crate::redirect::Redirect;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ImportedFd, LocalFd, ShortCStr};

/// `%var` source: clone the table fd above `min_fd`.
pub(super) fn resolve_var(
    var: &ShortCStr,
    export_to: i32,
    min_fd: i32,
    cell: &ForkCell<ShellState>,
) -> Result<Redirect, Report<OpenRedirectError>> {
    let state = cell.borrow().change_context(OpenRedirectError::Never)?;
    let local = state
        .fds
        .get(var)
        .ok_or_else(|| OpenRedirectError::VarNotFound { var: var.clone() })?
        .fd
        .try_clone_above(min_fd)
        .change_context(OpenRedirectError::Open)?;
    Ok(Redirect::Dup { export_to, local })
}

/// Path source: take the next pre-opened fd and clone it.
pub(super) fn resolve_path(
    export_to: i32,
    opened_iter: &mut core::slice::Iter<'_, LocalFd>,
) -> Result<Redirect, Report<OpenRedirectError>> {
    let local = opened_iter
        .next()
        .ok_or(OpenRedirectError::Open)?
        .try_clone()
        .change_context(OpenRedirectError::Open)?;
    Ok(Redirect::Dup { export_to, local })
}

/// `N>&M` source: dup the imported fd.
pub(super) fn resolve_dup(
    export_to: i32,
    from: i32,
) -> Result<Redirect, Report<OpenRedirectError>> {
    let imported = ImportedFd::from_number(from)
        .change_context_lazy(|| OpenRedirectError::FdNotOpen { n: from })?;
    let local = imported
        .try_dup()
        .change_context_lazy(|| OpenRedirectError::DupFdFailed { n: from })?;
    Ok(Redirect::Dup { export_to, local })
}
