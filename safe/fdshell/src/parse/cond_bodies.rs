//! Extract the heredoc body regions a token slice claims from its text. The
//! bodies sit after the command line (in the text), in operator order, and the
//! slice claims only the regions its own operators write: the operators
//! written on the slice's physical line **before** the slice already claimed
//! the head of the stream. Consumed by a block condition (whose bodies
//! `cond_bodies` hands to the cond-list run) and by a token slice whose text
//! is rebuilt from its regions (`part_text`, an arm body).

use super::Token;
use super::heredoc::operators;
use super::semi::token_range;
use crate::scan::heredoc::{
    Operator, body_regions_full, line_end_after, operator_count, operator_delims,
};
use alloc::vec::Vec;
use sys::ScriptText;
use sys::ShortCStr;

/// The condition's heredoc bodies (body lines plus the delimiter line, in
/// operator order), extracted from the block `text`. Empty when the condition
/// has no `<<` operators or the bodies are missing.
pub(crate) fn condition_bodies(text: &ScriptText, cond_tokens: &[Token]) -> Vec<ShortCStr> {
    let line = match text.as_bytes() {
        Ok(line) => line,
        Err(_) => return Vec::new(),
    };
    claimed_regions(text, cond_tokens)
        .into_iter()
        .filter_map(|(s, e)| {
            let bytes = line.get(s..e)?;
            ShortCStr::from_vec(bytes.to_vec()).ok()
        })
        .collect()
}

/// The regions of `text` that `tokens`' own `<<` operators claim (body lines
/// plus the delimiter line, in operator order). Empty when the slice has no
/// operator or the bodies are missing.
pub(crate) fn claimed_regions(text: &ScriptText, tokens: &[Token]) -> Vec<(usize, usize)> {
    let line = match text.as_bytes() {
        Ok(line) => line,
        Err(_) => return Vec::new(),
    };
    // The token offsets are relative to the full block text, so the
    // token-level operator scan runs over `line`, not a slice subslice.
    let (start, _) = token_range(tokens);
    // An empty operator list yields an empty region list through `take(ops.len())`
    // below — no separate guard.
    let ops = match operators(line, tokens) {
        Ok(ops) => ops,
        _ => return Vec::new(),
    };
    let line_end = line_end_after(line, start);
    let regions = match body_regions_full(line, line_end, &all_operators(line, line_end, start)) {
        Ok((regions, _)) => regions,
        Err(_) => return Vec::new(),
    };
    // Regions already claimed on the slice's own physical line by the
    // operators written before it (an earlier arm's heredoc).
    let claimed_before = operator_count(line, line_start_of(line, start), start);
    regions
        .into_iter()
        .skip(claimed_before)
        .take(ops.len())
        .collect()
}

/// Every `<<` operator on the slice's physical line, in operator order (empty
/// when the line has none).
fn all_operators(line: &[u8], line_end: usize, start: usize) -> Vec<Operator> {
    operator_delims(line, line_start_of(line, start), line_end).unwrap_or_default()
}

/// The start of the physical line that contains `pos`.
fn line_start_of(line: &[u8], pos: usize) -> usize {
    line.get(0..pos)
        .and_then(|head| head.iter().rposition(|&b| b == b'\n').map(|n| n + 1))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
