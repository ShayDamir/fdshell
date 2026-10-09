//! The pattern operator family (`#`, `##`, `%`, `%%`): anchored removal of the
//! shortest/longest prefix (or suffix) of the parameter's value that matches the
//! word pattern. A word pattern is `*`, `?`, `[...]`, `\X`, and quoted bytes; it
//! is never expanded (task #174), has no `FNM_PERIOD` dot rule, and `set -f`
//! (noglob) does not affect it.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::glob::match_component;
use crate::state::ShellState;
use crate::substitute::borrow_state;

use super::ParamOp;

/// Applies a pattern operator. No match leaves the value whole (rc 0); an unset
/// parameter pushes nothing, and `set -u` bails inside `param_value`.
pub(super) fn apply(
    name: &ShortCStr,
    op: ParamOp,
    word: &ShortCStr,
    mask: &[bool],
    content_start: usize,
    cell: &ForkCell<ShellState>,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    let state = borrow_state(cell)?;
    if let Some(value) = state.param_value(name)? {
        let pat = word.as_bytes().change_context(ResolveError::Never)?;
        let start = content_start + name.len() + op.word_offset();
        let k = strip_len(
            value.as_bytes().change_context(ResolveError::Never)?,
            pat,
            &pattern_mask(mask, start, pat.len()),
            op.longest(),
            op.prefix(),
        );
        let rest = if op.prefix() {
            value.get(k..)
        } else {
            value.get(..value.len() - k)
        };
        out.push(rest.ok_or(ResolveError::Never)?);
    }
    Ok(())
}

/// The pattern bytes' quote mask: the word-mask slice starting at `start` (the
/// word index of the first pattern byte), padded with `false` when the mask is
/// shorter than the pattern. The tokenizer strips `"` bytes from the word text,
/// so a quoted pattern byte is a masked byte, never a `"` byte.
fn pattern_mask(mask: &[bool], start: usize, len: usize) -> Vec<bool> {
    let mut out = Vec::new();
    for i in 0..len {
        out.push(mask.get(start + i).is_some_and(|&q| q));
    }
    out
}

/// Length of the anchored prefix (`prefix`) or suffix of `value` that matches
/// `pat` in full, `0` when nothing matches (so nothing is stripped). Shortest
/// scans candidate lengths ascending, longest descending, first hit wins.
fn strip_len(value: &[u8], pat: &[u8], pat_mask: &[bool], longest: bool, prefix: bool) -> usize {
    let n = value.len();
    for cand in 0..=n {
        let k = if longest { n - cand } else { cand };
        let candidate = if prefix {
            value.get(..k)
        } else {
            value.get(n - k..)
        };
        if candidate.is_some_and(|c| match_component(pat, pat_mask, c, true)) {
            return k;
        }
    }
    0
}
