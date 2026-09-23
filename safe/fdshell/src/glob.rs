//! Glob (pathname) expansion: a pattern word becomes its sorted matches.
//!
//! A word expands when it holds an unquoted, unescaped `*`, `?`, or valid
//! bracket expression. A pattern with no matches is passed through verbatim
//! unless `nullglob` is set, in which case it expands to nothing.

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

/// Glob-expand `word` (with its quote `mask`) into result words.
///
/// A word with no unquoted pattern bytes — and the empty word — passes
/// through unchanged. A pattern with no matches passes through unchanged
/// unless `nullglob` is set. Results are bytewise-sorted full paths; callers
/// assign any trace origin (matches are new shell words, a pass-through is
/// the input word itself).
pub(crate) fn expand(
    word: &ShortCStr,
    mask: &[bool],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    let bytes = word.as_bytes().change_context(ResolveError::Never)?;
    if !r#match::has_unquoted_pattern(bytes, mask) {
        return Ok(vec![word.clone()]);
    }
    let names = walk::walk(word, mask);
    if names.is_empty() {
        let nullglob = {
            let state = cell.borrow().change_context(ResolveError::RefNotFound)?;
            state.options & options::NULLGLOB != 0
        };
        if !nullglob {
            return Ok(vec![word.clone()]);
        }
        return Ok(Vec::new());
    }
    Ok(names
        .into_iter()
        .filter_map(|n| ShortCStr::from_vec(n).ok())
        .collect())
}

#[cfg(test)]
mod tests;
