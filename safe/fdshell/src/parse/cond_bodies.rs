//! Extract the heredoc bodies a token slice claims from its text. The bodies
//! sit after the command line (in the text), in operator order, and the slice
//! claims only the regions its own operators write: the operators written on
//! the slice's physical line **before** the slice already claimed the head of
//! the stream. Consumed by a block condition (whose bodies `condition_bodies`
//! hands to the cond-list run) and by a token slice whose text is rebuilt from
//! its body bytes (`part_text`, an arm body).

use super::Token;
use super::heredoc::operators;
use super::semi::token_range;
use crate::scan::heredoc::{
    Operator, body_contents, body_regions_full, line_end_after, line_start, operator_count,
    operator_delims,
};
use alloc::vec::Vec;
use sys::ScriptText;
use sys::ShortCStr;

/// The condition's heredoc bodies (body lines plus the delimiter line, in
/// operator order), extracted from the block `text`. Empty when the condition
/// has no `<<` operators, or when the line's **first** delimiter line is
/// missing (the allocation claims nothing, and the parse layer reports it).
pub(crate) fn condition_bodies(text: &ScriptText, cond_tokens: &[Token]) -> Vec<ShortCStr> {
    claimed(text, cond_tokens)
        .1
        .into_iter()
        .filter_map(|b| ShortCStr::from_vec(b).ok())
        .collect()
}

/// The regions of `text` that `tokens`' own `<<` operators claim (body lines
/// plus the delimiter line, in operator order). Empty when the slice has no
/// operator or the line's first delimiter line is missing.
pub(crate) fn claimed_regions(text: &ScriptText, tokens: &[Token]) -> Vec<(usize, usize)> {
    claimed(text, tokens).0
}

/// The body **bytes** of the same regions: a region whose delimiter line never
/// arrived carries that line synthesized, so the command line rebuilt from
/// these bytes parses as a complete heredoc (task #275).
pub(crate) fn claimed_bodies(text: &ScriptText, tokens: &[Token]) -> Vec<Vec<u8>> {
    claimed(text, tokens).1
}

/// The claimed regions **and** their bytes, in operator order: the ranges feed
/// `part_text`'s append/extension discriminator, the bytes feed the rebuild.
/// One entry per operator keeps the `skip`/`take` below a single arithmetic
/// unit with the window (LESSONS).
fn claimed(text: &ScriptText, tokens: &[Token]) -> (Vec<(usize, usize)>, Vec<Vec<u8>>) {
    let Ok(line) = text.as_bytes() else {
        return (Vec::new(), Vec::new());
    };
    // The token offsets are relative to the full block text, so the
    // token-level operator scan runs over `line`, not a slice subslice.
    let (start, _) = token_range(tokens);
    // An empty operator list yields an empty region list through `take(ops.len())`
    // below — no separate guard.
    let Ok(ops) = operators(line, tokens) else {
        return (Vec::new(), Vec::new());
    };
    let line_end = line_end_after(line, start);
    let all = all_operators(line, line_end, start);
    let alloc = body_regions_full(line, line_end, &all);
    if alloc.regions.is_empty() {
        return (Vec::new(), Vec::new());
    }
    // Regions already claimed on the slice's own physical line by the
    // operators written before it (an earlier arm's heredoc).
    let claimed_before = operator_count(line, line_start(line, start), start);
    let regions = alloc
        .regions
        .iter()
        .copied()
        .skip(claimed_before)
        .take(ops.len())
        .collect();
    let bodies = body_contents(line, &alloc)
        .into_iter()
        .skip(claimed_before)
        .take(ops.len())
        .collect();
    (regions, bodies)
}

/// Every `<<` operator on the slice's physical line, in operator order (empty
/// when the line has none).
fn all_operators(line: &[u8], line_end: usize, start: usize) -> Vec<Operator> {
    operator_delims(line, line_start(line, start), line_end).unwrap_or_default()
}

#[cfg(test)]
mod tests;
