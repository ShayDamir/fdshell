//! The here-source arms (`<<<word`, `<<word`): the body bytes become a memfd,
//! which is then cloned above every target of the command like every other
//! source. Split out of `redirect/resolve.rs` for the line budget (STYLE.md
//! §2.2); the two arms share nothing with the fd-state arms. `resolve_one`
//! passes the **matched variant's fields**, so neither arm has to reject a
//! source that is not a here-body (no impossible arm, no coverage hole).

use crate::error::redirect::OpenRedirectError;
use crate::mask::MaskBit;
use crate::redirect::{Redirect, RedirectDef, heredoc, herestring};
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::fork_cell::ForkCell;
use sys::{ExportedFd, LocalFd, ShortCStr};

/// The shape both arms return: one `Redirect` and no pair, because a here-body
/// redirection has no `extra_to`. The memfd is cloned above every target of the
/// command, as every other source is (LESSONS: "any new source-allocating
/// redirect arm must take the same minimum").
fn one(
    def: &RedirectDef,
    body: LocalFd,
    min_fd: i32,
    ctx: OpenRedirectError,
) -> Result<(Redirect, Option<Redirect>), Report<OpenRedirectError>> {
    let local = body.try_clone_above(min_fd).change_context(ctx)?;
    Ok((Redirect::new(def.export_to, local), None))
}

/// `N<<<word`: the word is expanded with its own quote mask, and the body is
/// the expansion plus a newline.
pub(super) fn resolve_string(
    def: &RedirectDef,
    word: &ShortCStr,
    mask: &[MaskBit],
    spans: &[usize],
    min_fd: i32,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<(Redirect, Option<Redirect>), Report<OpenRedirectError>> {
    let body = herestring::here_string(word, mask, spans, cache, cell)?;
    one(def, body, min_fd, OpenRedirectError::HereStringCreate)
}

/// `N<<word`: the body bytes, verbatim for a quoted delimiter and expanded for
/// an unquoted one, with no trailing newline added.
pub(super) fn resolve_doc(
    def: &RedirectDef,
    body: &ShortCStr,
    expand: bool,
    min_fd: i32,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<(Redirect, Option<Redirect>), Report<OpenRedirectError>> {
    let data = heredoc::here_doc(body, expand, cache, cell)?;
    one(def, data, min_fd, OpenRedirectError::HereDocCreate)
}
