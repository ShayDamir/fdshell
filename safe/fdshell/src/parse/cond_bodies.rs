//! Extract a block condition's heredoc body bytes from the block text. The
//! bodies sit after the condition line (in the block text), in operator order,
//! and the condition claims only the regions its own operators write: the
//! operators written on the condition's physical line **before** the condition
//! already claimed the head of the stream.

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
    // The token offsets are relative to the full block text, so the
    // token-level operator scan runs over `line`, not a condition subslice.
    let (cond_start, _) = token_range(cond_tokens);
    // An empty operator list yields an empty body list through `take(ops.len())`
    // below — no separate guard.
    let ops = match operators(line, cond_tokens) {
        Ok(ops) => ops,
        _ => return Vec::new(),
    };
    let line_end = line_end_after(line, cond_start);
    let regions =
        match body_regions_full(line, line_end, &all_operators(line, line_end, cond_start)) {
            Ok((regions, _)) => regions,
            Err(_) => return Vec::new(),
        };
    // Regions already claimed on the condition's own physical line by the
    // operators written before it (an earlier arm's heredoc).
    let claimed_before = operator_count(line, line_start_of(line, cond_start), cond_start);
    regions
        .into_iter()
        .skip(claimed_before)
        .take(ops.len())
        .filter_map(|(s, e)| {
            let bytes = line.get(s..e)?;
            ShortCStr::from_vec(bytes.to_vec()).ok()
        })
        .collect()
}

/// Every `<<` operator on the condition's physical line, in operator order
/// (empty when the line has none).
fn all_operators(line: &[u8], line_end: usize, cond_start: usize) -> Vec<Operator> {
    operator_delims(line, line_start_of(line, cond_start), line_end).unwrap_or_default()
}

/// The start of the physical line that contains `pos`.
fn line_start_of(line: &[u8], pos: usize) -> usize {
    line.get(0..pos)
        .and_then(|head| head.iter().rposition(|&b| b == b'\n').map(|n| n + 1))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
