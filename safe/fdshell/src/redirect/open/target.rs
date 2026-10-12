//! The redirect *target* word: fold its escape pairs, apply the strict
//! absolute ban, and glob it.

use crate::bytes::fold::fold_word;
use crate::error::redirect::OpenRedirectError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt, bail};
use sys::LocalFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// The opened-target name for one redirect path source: `> 1` match is
/// ambiguous, `0` matches open the literal word (bash passes unmatched
/// redirect patterns through; `failglob` does not apply to redirects), `1`
/// match opens that match.
///
/// The word is folded first (POSIX #4.1): bash folds every unquoted escape
/// pair at tokenization, and a redirect target is never substituted, so
/// `> a\*b` opens the file named `a*b`. A `base` is strict mode's pinned
/// `%CWD` fd: the walk is rooted there and an absolute target (filesystem-root
/// resolution) is banned *before* the walk, so a leading-`/` pattern never
/// reaches the root. `None` is the non-strict process-CWD route, unchanged.
pub(super) fn resolve(
    path: &ShortCStr,
    mask: &[bool],
    base: Option<&LocalFd>,
    cell: &ForkCell<ShellState>,
) -> Result<ShortCStr, Report<OpenRedirectError>> {
    let (target, target_mask) = fold_word(path, mask).change_context(OpenRedirectError::Never)?;
    if base.is_some() && target.starts_with(b"/") {
        bail!(OpenRedirectError::StrictAbsoluteTarget {
            name: target.clone(),
        });
    }
    let matches = crate::glob::matches(&target, &target_mask, cell, base)
        .change_context(OpenRedirectError::Glob)?;
    match matches.len() {
        0 => Ok(target),
        1 => matches
            .first()
            .cloned()
            .ok_or(OpenRedirectError::Never.into()),
        _ => Err(Report::new(OpenRedirectError::AmbiguousRedirect {
            name: path.clone(),
        })),
    }
}
