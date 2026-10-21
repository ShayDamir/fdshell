//! Word-level substitution: per-argument expansion, IFS splitting, globbing.

use alloc::vec::Vec;
use error_stack::Report;
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::state::ShellState;

use super::fields::word_fields;

/// The argument pipeline, one word at a time: [`word_fields`] expands the word
/// and splits it on IFS, and every field is pathname-expanded **here** —
/// CWD-rooted, with the `nullglob`/`failglob` policy (`glob::expand`). The one
/// fd `cache` is shared by every word of the call, as the `%fd` lookups repeat.
pub fn substitute_args(
    args: &[ShortCStr],
    args_mask: &[Vec<MaskBit>],
    args_span: &[Vec<usize>],
    args_quoted: &[bool],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ShortCStr>, Report<ResolveError>> {
    let mut result = Vec::new();
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    for (i, arg) in args.iter().enumerate() {
        let mask = args_mask.get(i).cloned().unwrap_or_default();
        let spans = args_span.get(i).cloned().unwrap_or_default();
        let quoted = args_quoted.get(i).copied().unwrap_or(false);
        for (word, word_mask) in word_fields(arg, &mask, &spans, quoted, &mut cache, cell)? {
            push_word(&mut result, &word, &word_mask, cell)?;
        }
    }
    Ok(result)
}

/// One word after IFS splitting: a fully quoted word is kept as-is (even
/// empty), any other word is pathname-expanded first.
fn push_word(
    result: &mut Vec<ShortCStr>,
    word: &ShortCStr,
    mask: &[MaskBit],
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<ResolveError>> {
    if super::fully_quoted(mask) {
        result.push(word.clone());
        return Ok(());
    }
    for expanded in crate::glob::expand(word, mask, cell)? {
        result.push(expanded);
    }
    Ok(())
}
