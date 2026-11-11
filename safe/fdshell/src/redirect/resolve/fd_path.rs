//! The `/dev/fd/<n>` and `/proc/self/fd/<n>` redirect targets: Linux's magic
//! paths. bash opens them with the operator's flags **as the redirection runs**,
//! so the open sees the fd state the preceding redirections left
//! (`redirect/resolve/table.rs`) and yields a **fresh** open file description of
//! the target file — its own offset, `>` truncating it. Measured against bash
//! 5.3.9: `f > z 2> /dev/fd/1` leaves `z` = `e\n\n` (two descriptions racing at
//! 0), not the `hi\ne\n` a `dup` shares, and `cat < /dev/fd/0` after a partial
//! read starts at byte 0 (task #337). The magic reading runs on the **final
//! expanded name** — the open pass flags such defs on the name it resolves
//! (`redirect/open/target.rs`), so `2> $p` with `p=/dev/fd/1` is opened here too
//! (task #479) — and the pass opens nothing for a flagged def on the plain arm:
//! this open is the def's own, run at its point in the walk, so the table
//! answers its **success** too — a magic name an earlier def's fd makes
//! reachable opens like bash's (task #482). The pass keeps only the noclobber
//! gate's arms on the expanded word.

// mutants: -C --lib -C --test -C redirect -- -E 'test(/^redirect::resolve::/) | binary(redirect)'

use super::{arms, table};
use crate::error::redirect::OpenRedirectError;
use crate::redirect::{Redirect, RedirectDef};
use error_stack::{Report, ResultExt};

/// Resolve a magic-path def against the fd state the preceding defs leave: the
/// number the table resolves for `n` — a preceding def's own fd, else `n`
/// itself — opened under `prefix` with the def's flags. A preceding `N>&-`
/// leaves no fd and the kernel has no entry for a closed one, so the def fails
/// on the open, as bash's does.
pub(super) fn resolve(
    def: &RedirectDef,
    prefix: &str,
    n: i32,
    min_fd: i32,
    built: &[Redirect],
    seed: &[Redirect],
) -> Result<(Redirect, Option<Redirect>), Report<OpenRedirectError>> {
    let raw = table::number(n, built, seed).ok_or_else(|| Report::new(OpenRedirectError::Open))?;
    let word = sys::format!("{prefix}{raw}").change_context(OpenRedirectError::Open)?;
    let src = sys::openat2::open(word.export(), def.direction.open_flags())
        .change_context(OpenRedirectError::Open)?;
    let local = src
        .try_clone_above(min_fd)
        .change_context(OpenRedirectError::Open)?;
    let pair = arms::pair_of(&src, def.extra_to, min_fd)?;
    Ok((Redirect::new(def.export_to, local), pair))
}
