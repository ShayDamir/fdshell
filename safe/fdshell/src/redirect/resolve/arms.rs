// mutants: -C --lib -C --test -C redirect -- -E 'test(/^redirect::resolve::/) | binary(redirect)'

use super::{fd_path, table};
use crate::error::redirect::OpenRedirectError;
use crate::redirect::{OpenedTarget, Redirect, RedirectDef};
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;
use sys::{LocalFd, ShortCStr};

/// A def with `extra_to` yields a second `Redirect` fed by a **second clone of
/// the same source fd**: both targets share one open file description, so they
/// append to each other (bash's `> f 2>&1`). The clone takes the same `min_fd`
/// minimum, so it never lands on a target fd (LESSONS).
pub(super) fn pair_of(
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

/// Path source: take the open pass's entry and clone its fd above the targets —
/// except when the entry is flagged magic: the open pass decided that on the
/// final expanded name (`redirect/open/target.rs`), and such a def's open
/// belongs to the fd state the preceding defs leave
/// (`redirect/resolve/fd_path.rs`). The entry's fd is then left to its owner:
/// it is CLOEXEC and above every target, so the command never sees it, and its
/// open ran against the descriptor the shell had **before** the command's
/// redirections (task #337).
pub(super) fn resolve_path(
    def: &RedirectDef,
    entry: &OpenedTarget,
    min_fd: i32,
    built: &[Redirect],
    seed: &[Redirect],
) -> Result<(Redirect, Option<Redirect>), Report<OpenRedirectError>> {
    if let Some((prefix, n)) = entry.magic {
        return fd_path::resolve(def, prefix, n, min_fd, built, seed);
    }
    let local = entry
        .fd
        .try_clone_above(min_fd)
        .change_context(OpenRedirectError::Open)?;
    let pair = pair_of(&entry.fd, def.extra_to, min_fd)?;
    Ok((Redirect::new(def.export_to, local), pair))
}

/// `N>&M` source: the descriptor the command's **preceding** redirections left
/// on `M` (`redirect/resolve/table.rs`), cloned above every target of the
/// command. `cmd > f 2>&1` therefore lands fd 2 on `f`'s open file description,
/// sharing its offset, exactly as bash's `dup2` of the post-redirect fd 1 does.
pub(super) fn resolve_dup(
    export_to: i32,
    min_fd: i32,
    from: i32,
    built: &[Redirect],
    seed: &[Redirect],
) -> Result<(Redirect, Option<Redirect>), Report<OpenRedirectError>> {
    let local = table::source(from, built, seed, min_fd)?;
    Ok((Redirect::new(export_to, local), None))
}
