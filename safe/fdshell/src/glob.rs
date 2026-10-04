//! Glob (pathname) expansion: a pattern word becomes its sorted matches.
//!
//! A word expands when it holds an unquoted, unescaped `*`, `?`, or valid
//! bracket expression. `matches` returns the raw match list; `expand` applies
//! the shell-option policy on an empty result (`failglob` errors, `nullglob`
//! empties, otherwise the word passes through).

mod r#match;
mod walk;

use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::options;
use crate::state::ShellState;

/// Raw pathname matches of `word` (with its quote `mask`), bytewise-sorted.
///
/// A word with no unquoted pattern bytes — and the empty word — yields
/// `[word]` without touching the filesystem; a pattern with no matches yields
/// `[]`. `dotglob` is read from `cell` (block-scoped borrow) and threaded
/// into the walk.
pub(crate) fn matches(
    word: &ShortCStr,
    mask: &[bool],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    let bytes = word.as_bytes().change_context(ResolveError::Never)?;
    if !r#match::has_unquoted_pattern(bytes, mask) {
        return Ok(vec![word.clone()]);
    }
    let (dotglob, noglob) = {
        let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
        (
            state.options & options::DOTGLOB != 0,
            state.options & options::NOGLOB != 0,
        )
    };
    if noglob {
        // `set -f`: the pattern word passes through verbatim (no filesystem).
        return Ok(vec![word.clone()]);
    }
    Ok(walk::walk(word, mask, dotglob)
        .into_iter()
        .filter_map(|n| ShortCStr::from_vec(n).ok())
        .collect())
}

/// Glob-expand `word` into result words, applying the option policy on a
/// pattern with no matches: `failglob` (checked first) errors, `nullglob`
/// empties the result, otherwise the word passes through unchanged. A
/// non-pattern word is one word (its own clone); matches are bytewise-sorted
/// full paths (callers assign any trace origin).
pub(crate) fn expand(
    word: &ShortCStr,
    mask: &[bool],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    let names = matches(word, mask, cell)?;
    if !names.is_empty() {
        return Ok(names);
    }
    // `matches` released the cell borrow; re-borrow for the option policy.
    let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
    let opts = state.options;
    if opts & options::FAILGLOB != 0 {
        return Err(Report::new(ResolveError::GlobNoMatch {
            word: word.clone(),
        }));
    }
    if opts & options::NULLGLOB != 0 {
        return Ok(Vec::new());
    }
    Ok(vec![word.clone()])
}

#[cfg(test)]
mod tests;
