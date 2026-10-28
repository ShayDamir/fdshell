//! The here-source arms (`<<<word`, `<<word`): the body bytes become a memfd,
//! which is then cloned above every target of the command like every other
//! source. Split out of `redirect/resolve.rs` for the line budget (STYLE.md
//! §2.2); the two arms share nothing with the fd-state arms.

use crate::error::redirect::OpenRedirectError;
use crate::redirect::{Redirect, RedirectDef, RedirectSource, heredoc, herestring};
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::fork_cell::ForkCell;
use sys::{ExportedFd, ShortCStr};

/// The here-sources, in the shape `resolve_one` wants (a single `Redirect`, no
/// pair: a here-body redirection has no `extra_to`).
pub(super) fn resolve(
    def: &RedirectDef,
    source: &RedirectSource,
    min_fd: i32,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<(Redirect, Option<Redirect>), Report<OpenRedirectError>> {
    let local = match source {
        RedirectSource::HereString { word, mask, spans } => {
            let body = herestring::here_string(word, mask, spans, cache, cell)?;
            body.try_clone_above(min_fd)
                .change_context(OpenRedirectError::HereStringCreate)?
        }
        RedirectSource::HereDoc { body, expand } => {
            let data = heredoc::here_doc(body, *expand, cache, cell)?;
            data.try_clone_above(min_fd)
                .change_context(OpenRedirectError::HereDocCreate)?
        }
        _ => return Err(Report::new(OpenRedirectError::Never)),
    };
    Ok((Redirect::new(def.export_to, local), None))
}
