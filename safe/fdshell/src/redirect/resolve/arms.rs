use crate::error::redirect::OpenRedirectError;
use crate::redirect::Redirect;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{ImportedFd, LocalFd, ShortCStr};

/// A def with `extra_to` yields a second `Redirect` fed by a **second clone of
/// the same source fd**: both targets share one open file description, so they
/// append to each other (bash's `> f 2>&1`). The clone takes the same `min_fd`
/// minimum, so it never lands on a target fd (LESSONS).
fn pair_of(
    src: &LocalFd,
    extra_to: Option<i32>,
    min_fd: i32,
) -> Result<Option<Redirect>, Report<OpenRedirectError>> {
    let Some(target) = extra_to else {
        return Ok(None);
    };
    let local = src
        .try_clone_above(min_fd)
        .change_context(OpenRedirectError::Open)?;
    Ok(Some(Redirect::new(target, local)))
}

/// `%var` source: clone the table fd above `min_fd`.
pub(super) fn resolve_var(
    var: &ShortCStr,
    export_to: i32,
    extra_to: Option<i32>,
    min_fd: i32,
    cell: &ForkCell<ShellState>,
) -> Result<(Redirect, Option<Redirect>), Report<OpenRedirectError>> {
    let state = cell.borrow().change_context(OpenRedirectError::Never)?;
    let src = &state
        .fds
        .get(var)
        .ok_or_else(|| OpenRedirectError::VarNotFound { var: var.clone() })?
        .fd;
    let local = src
        .try_clone_above(min_fd)
        .change_context(OpenRedirectError::Open)?;
    let pair = pair_of(src, extra_to, min_fd)?;
    Ok((Redirect::new(export_to, local), pair))
}

/// Path source: take the next pre-opened fd and clone it above the targets.
pub(super) fn resolve_path(
    export_to: i32,
    extra_to: Option<i32>,
    min_fd: i32,
    opened_iter: &mut core::slice::Iter<'_, LocalFd>,
) -> Result<(Redirect, Option<Redirect>), Report<OpenRedirectError>> {
    let src = opened_iter.next().ok_or(OpenRedirectError::Open)?;
    let local = src
        .try_clone_above(min_fd)
        .change_context(OpenRedirectError::Open)?;
    let pair = pair_of(src, extra_to, min_fd)?;
    Ok((Redirect::new(export_to, local), pair))
}

/// `N>&M` source: dup the imported fd above the targets.
pub(super) fn resolve_dup(
    export_to: i32,
    min_fd: i32,
    from: i32,
) -> Result<(Redirect, Option<Redirect>), Report<OpenRedirectError>> {
    let imported = ImportedFd::from_number(from)
        .change_context_lazy(|| OpenRedirectError::FdNotOpen { n: from })?;
    let local = imported
        .try_dup_above(min_fd)
        .change_context_lazy(|| OpenRedirectError::DupFdFailed { n: from })?;
    Ok((Redirect::new(export_to, local), None))
}
