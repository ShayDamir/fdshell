mod arms;
mod fd_path;
mod here;
mod table;

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

/// Turn a command's defs into the `Redirect`s a consumer exports, in the order
/// the redirections were written. `seed` is the fd state the caller has already
/// installed: a pipeline child passes the pipe ends it cloned for the stage,
/// because bash has those fds in place when it applies the stage's `2>&1`, and
/// every other caller passes `&[]`. A `Dup` source and a `/dev/fd/N` target
/// resolve against the state the **preceding** defs leave, not against the
/// process's fds (`redirect/resolve/table.rs`).
pub fn resolve_redirects(
    redirects: &[RedirectDef],
    opened: &[LocalFd],
    seed: &[Redirect],
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
        let (one, pair) = resolve_one(
            r,
            &mut opened_iter,
            &mut cache,
            cell,
            min_fd,
            seed,
            &resolved,
        )?;
        resolved.push(one);
        if let Some(pair) = pair {
            resolved.push(pair);
        }
    }
    Ok(resolved)
}

/// One def, in the fd state its predecessors leave: `built` is the resolved
/// prefix and `seed` the fds the caller installed before the command. A def
/// with `extra_to` yields a pair, and the pair is pushed after its fd-1 twin so
/// the two targets keep the order the operator implies.
fn resolve_one(
    r: &RedirectDef,
    opened_iter: &mut core::slice::Iter<'_, LocalFd>,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
    min_fd: i32,
    seed: &[Redirect],
    built: &[Redirect],
) -> Result<(Redirect, Option<Redirect>), Report<OpenRedirectError>> {
    match &r.source {
        super::RedirectSource::Var(var) => {
            arms::resolve_var(var, r.export_to, r.extra_to, min_fd, cell)
        }
        super::RedirectSource::Path { path, .. } => {
            arms::resolve_path(r, path, min_fd, opened_iter, built, seed)
        }
        super::RedirectSource::HereString { word, mask, spans } => {
            here::resolve_string(r, word, mask, spans, min_fd, cache, cell)
        }
        super::RedirectSource::HereDoc { body, expand } => {
            here::resolve_doc(r, body, *expand, min_fd, cache, cell)
        }
        super::RedirectSource::Dup(from) => {
            arms::resolve_dup(r.export_to, min_fd, *from, built, seed)
        }
        super::RedirectSource::Close => Ok((
            Redirect::Close {
                export_to: r.export_to,
            },
            None,
        )),
    }
}

#[cfg(test)]
mod tests;
