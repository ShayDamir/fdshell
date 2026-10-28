//! The fd state a redirect source resolves against: what the command's
//! **earlier** redirections leave on each fd. POSIX #2.5 applies a command's
//! redirections one by one in the order written, so `cmd > f 2>&1` dups the fd
//! 1 that `> f` installed, not the shell's (LESSONS: "POSIX redirect order is
//! observable").
//!
//! The table is the slice of already-built [`Redirect`]s, not a map: a
//! redirection's entry is the descriptor it will export, so cloning that
//! descriptor (`F_DUPFD_CLOEXEC`) shares the same open file description and its
//! offset, which is exactly bash's `dup2` of the post-redirect fd. A pipeline
//! child seeds the table with its pipe ends, because bash has those in place
//! when it applies a stage's `2>&1`.

use crate::error::redirect::OpenRedirectError;
use crate::redirect::Redirect;
use error_stack::{Report, ResultExt};
use sys::{ImportedFd, LocalFd};

/// The entry that wins fd `from`: the **last** one in `entries`, since a later
/// redirection to the same fd replaces the earlier one.
pub(super) fn latest(from: i32, entries: &[Redirect]) -> Option<&Redirect> {
    entries.iter().rev().find(|r| r.target() == from)
}

/// The descriptor a `Dup(from)` source — a `/dev/fd/<from>` target goes through
/// the same lookup — resolves to: the entry the preceding redirections left on
/// `from`, cloned above every target of this command; `FdNotOpen` when a
/// preceding `N>&-` closed it; and the inherited **process** fd `from` when no
/// redirection touched it, which is bash's plain `dup` of an untouched fd.
pub(super) fn source(
    from: i32,
    built: &[Redirect],
    seed: &[Redirect],
    min_fd: i32,
) -> Result<LocalFd, Report<OpenRedirectError>> {
    match latest(from, built).or_else(|| latest(from, seed)) {
        Some(Redirect::Dup { local, .. }) => local
            .try_clone_above(min_fd)
            .change_context(OpenRedirectError::Open),
        Some(Redirect::Close { .. }) => Err(Report::new(OpenRedirectError::FdNotOpen { n: from })),
        None => dup_process(from, min_fd),
    }
}

/// The fd **number** a `/dev/fd/<from>` target opens through: the local fd of the
/// entry the preceding redirections left on `from` — opening `/dev/fd/<that>`
/// follows the magic symlink to the file the def's `open()` wants — and `from`
/// itself when no redirection touched it. `None` for a preceding `N>&-`: the fd
/// is gone and the kernel has no `/dev/fd/<from>` entry for it, which is the
/// failed open bash measures.
pub(super) fn number(from: i32, built: &[Redirect], seed: &[Redirect]) -> Option<i32> {
    match latest(from, built).or_else(|| latest(from, seed)) {
        Some(Redirect::Dup { local, .. }) => Some(local.as_raw()),
        Some(Redirect::Close { .. }) => None,
        None => Some(from),
    }
}

/// A fd the command never redirected: copy the fd the shell passed down.
fn dup_process(from: i32, min_fd: i32) -> Result<LocalFd, Report<OpenRedirectError>> {
    let imported = ImportedFd::from_number(from)
        .change_context_lazy(|| OpenRedirectError::FdNotOpen { n: from })?;
    imported
        .try_dup_above(min_fd)
        .change_context_lazy(|| OpenRedirectError::DupFdFailed { n: from })
}
