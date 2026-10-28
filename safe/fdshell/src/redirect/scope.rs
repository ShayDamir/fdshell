//! POSIX #2.4: a redirection belongs to the simple command it is written on,
//! and a simple command includes a user-function call and an in-process builtin.
//! `Scope` applies a command's redirections to the shell's own fds and puts the
//! fds back when the command finishes.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::{ImportedFd, LocalFd};

use super::RedirectDef;

/// The target fds of one command's redirections, saved in apply order.
pub struct Scope {
    saved: Vec<(i32, Option<LocalFd>)>,
}

impl Scope {
    /// Save each target fd, then apply the redirections in the order they were
    /// written (POSIX #2.5). "The last entry to a fd wins" is a consequence of
    /// that order, and every entry still opens/truncates its target, so a
    /// failing open aborts the command before its handler runs. The seed is
    /// empty: the shell's own fds are the state a scope's `Dup` source resolves
    /// against, and the scope's saves are the only other fd state (a pipeline
    /// stage passes its pipe ends instead — `pipeline/child.rs`).
    ///
    /// A saved copy is allocated strictly above every target of the command, so
    /// no `dup2` of a later redirection can clobber it; a closed target records
    /// `None` because `EBADF` is not an error the user can fix (§4.12). Saving
    /// rides the apply loop rather than pre-passing over the whole list, and
    /// that stays sound under the fd-state resolution: for a fd touched by
    /// several redirections the **first** save of that fd precedes its first
    /// export, so the shell's original value is the earliest copy and
    /// `restore`'s reverse iteration replays it last
    /// (`scope/tests.rs::interleaved_scope_restores_both_fds_after_a_dup`).
    pub fn open(
        redirects: &[RedirectDef],
        cell: &ForkCell<ShellState>,
    ) -> Result<Self, Report<CmdError>> {
        let opened =
            super::open_redirect_files(redirects, cell).change_context(CmdError::Redirect)?;
        let resolved = super::resolve_redirects(redirects, &opened, &[], cell)
            .change_context(CmdError::Redirect)?;
        let min_fd = RedirectDef::max_target(redirects)
            .checked_add(1)
            .ok_or(CmdError::Redirect)?;
        let mut saved = Vec::new();
        for r in &resolved {
            let target = r.target();
            saved.push((target, save(target, min_fd)));
            r.export().change_context(CmdError::Redirect)?;
        }
        Ok(Self { saved })
    }

    /// Run `f` with the scope open, then [`restore`] it on **every** exit path:
    /// `?` on success (a failed restore is the actionable error), best-effort on
    /// the report path, where the report is what the user can act on (LESSONS
    /// "the redirect scope restores in reverse apply order, on every exit
    /// path"). `run_group` and the keyword blocks all take this shape, so no
    /// call site hand-copies the match.
    pub fn run<T, F>(self, f: F) -> Result<T, Report<CmdError>>
    where
        F: FnOnce() -> Result<T, Report<CmdError>>,
    {
        match f() {
            Ok(value) => {
                self.restore()?;
                Ok(value)
            }
            Err(report) => {
                let _ = self.restore();
                Err(report)
            }
        }
    }

    /// Put the fds back in **reverse** apply order: the last redirection to a fd
    /// is its winner, so the winner's predecessor must be restored first
    /// (forward restore would leave the loser in charge).
    ///
    /// A saved copy is `CLOEXEC`, so a forked child inside the command never
    /// inherits it through `exec`; the copy is closed by its `Drop` (§5.9).
    pub fn restore(self) -> Result<(), Report<CmdError>> {
        for (target, copy) in self.saved.iter().rev() {
            match copy {
                Some(fd) => {
                    fd.export_to(*target).change_context(CmdError::Redirect)?;
                }
                // The target was closed when the scope opened, so closing it is
                // the restore; an already-closed fd is not an actionable error.
                None => {
                    let _ = sys::dup::close(*target);
                }
            }
        }
        Ok(())
    }
}

/// Copy the shell fd `target` into a fresh `CLOEXEC` fd above `min_fd`; `None`
/// when `target` is closed.
fn save(target: i32, min_fd: i32) -> Option<LocalFd> {
    ImportedFd::from_number(target)
        .ok()?
        .try_dup_above(min_fd)
        .ok()
}

#[cfg(test)]
mod tests;
