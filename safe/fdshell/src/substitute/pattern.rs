//! The `case` **pattern** word: expand it, then keep the word bash keeps.
//!
//! bash expands a pattern word as a word (`PATTERN` expansion) and uses only
//! its **first expanded word** as the pattern (`execute_case_command`,
//! bash-5.3 `execute_cmd.c`: the pattern is `es->word->word`), while it
//! performs **no** IFS splitting and **no** pathname expansion on it. So the
//! pattern surface is `substitute/arg.rs` followed by a cut that only the
//! `$@` word boundaries make, and a take-first (`Join::Pattern`, task #319).
//! LESSONS: "`$@` has two join rules".

use alloc::vec::Vec;
use error_stack::Report;
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::state::ShellState;

use super::Join;
use super::arg::substitute_arg_marked;
use super::split::split_word;

/// One `case` pattern clause element as the `(pattern text, parallel mask)`
/// `glob::match_component` matches with. The expanded text is cut on the
/// recorded `$@` word boundaries — never on `IFS` — and only the first piece
/// is the pattern; no piece at all (an expansion that produced nothing, e.g.
/// `"$@"` with no positionals) is the empty pattern with the empty mask, which
/// matches only the empty word.
pub(crate) fn word(
    text: &ShortCStr,
    mask: &[MaskBit],
    spans: &[usize],
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<(ShortCStr, Vec<MaskBit>), Report<ResolveError>> {
    let (text, text_mask, origin) =
        substitute_arg_marked(text, mask, spans, Join::Pattern, cache, cell)?;
    // The **empty** `IFS` is the boundary-only cut: with no `IFS` bytes,
    // `split_word` splits nothing and only the `$@` boundaries close a field.
    // This caller never reads the shell's `IFS` — the pattern's `IFS` reading
    // is the arm's (`dollar/at_star.rs`), so the two cannot drift apart.
    let fields = split_word(&text, &text_mask, &ShortCStr::new(), &origin)?;
    Ok(fields
        .into_iter()
        .next()
        .unwrap_or_else(|| (ShortCStr::new(), Vec::new())))
}

#[cfg(test)]
mod tests;
