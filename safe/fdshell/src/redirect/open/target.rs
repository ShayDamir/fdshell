//! The redirect *target* word: expand it when it carries a substitution
//! trigger, fold its escape pairs, apply the strict absolute ban, glob it, and
//! demand the one name bash's `ambiguous redirect` rule leaves.

mod expand;

use crate::bytes::fold::fold_word;
use crate::error::redirect::OpenRedirectError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt, bail};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use super::RedirectSource;

/// The opened-target name for one redirect path source. A word with an
/// expansion trigger goes through the **word pipeline** (task #278):
/// `$( … )`, `` ` … ` ``, `$(( … ))`, `$name`, `%fd` and `~` expand, the
/// result is split on IFS, and every field is globbed at the redirect's own
/// base. A word without one keeps the fold-and-glob path below.
///
/// Then bash's rule, once for both paths ([`one_name`]): exactly one name
/// opens, anything else is `ambiguous redirect`. A `base` is strict mode's
/// pinned `%CWD` fd: the walk is rooted there and an absolute target is banned
/// **before** the walk — on the word (no trigger) or on every *expanded* field
/// (trigger), so `> $(echo /etc/x)` never reaches the filesystem root under
/// strict mode. `None` is the non-strict process-CWD route, unchanged.
pub(super) fn resolve(
    source: &RedirectSource,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    base: Option<&LocalFd>,
    cell: &ForkCell<ShellState>,
) -> Result<ShortCStr, Report<OpenRedirectError>> {
    let RedirectSource::Path {
        path,
        mask,
        spans,
        quoted,
    } = source
    else {
        return Err(Report::new(OpenRedirectError::Never));
    };
    let bytes = path.as_bytes().change_context(OpenRedirectError::Never)?;
    if !crate::substitute::target_trigger(bytes) {
        return literal(path, mask, base, cell);
    }
    expand::target(path, mask, spans, *quoted, base, cache, cell)
}

/// The no-trigger fast path, byte-for-byte what a target was before word
/// substitution existed (POSIX #4.1): bash folds every unquoted escape pair at
/// tokenization, so `> a\*b` opens the file named `a*b`; then glob, and a
/// pattern with **no** match opens the pattern word itself (`zzz*` is a file
/// name, `failglob` does not apply to a redirect).
fn literal(
    path: &ShortCStr,
    mask: &[bool],
    base: Option<&LocalFd>,
    cell: &ForkCell<ShellState>,
) -> Result<ShortCStr, Report<OpenRedirectError>> {
    let (target, target_mask) = fold_word(path, mask).change_context(OpenRedirectError::Never)?;
    ban_absolute(&target, base)?;
    let mut matches = crate::glob::matches(&target, &target_mask, cell, base)
        .change_context(OpenRedirectError::Glob)?;
    if matches.is_empty() {
        matches.push(target);
    }
    one_name(&matches, path)
}

/// Strict mode's ban on filesystem-root resolution, evaluated on the target
/// word (the fast path) or on every **expanded** field (the substitution
/// path), always before the glob walk.
pub(super) fn ban_absolute(
    target: &ShortCStr,
    base: Option<&LocalFd>,
) -> Result<(), Report<OpenRedirectError>> {
    if base.is_some() && target.starts_with(b"/") {
        bail!(OpenRedirectError::StrictAbsoluteTarget {
            name: target.clone(),
        });
    }
    Ok(())
}

/// bash's redirect rule, shared by both paths: **one** name opens; 0 names
/// (`> $(true)`, a `nullglob`-deleted pattern) and many (`n="a b"; > $n`, a
/// two-match glob) are `ambiguous redirect`, reported with the word as written.
pub(super) fn one_name(
    names: &[ShortCStr],
    word: &ShortCStr,
) -> Result<ShortCStr, Report<OpenRedirectError>> {
    match names.len() {
        1 => names
            .first()
            .cloned()
            .ok_or(Report::new(OpenRedirectError::Never)),
        _ => Err(Report::new(OpenRedirectError::AmbiguousRedirect {
            name: word.clone(),
        })),
    }
}
