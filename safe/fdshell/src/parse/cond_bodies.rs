//! Extract a block condition's heredoc body bytes from the block text. The
//! bodies sit after the condition line (in the block text), in operator order.

use super::Token;
use super::heredoc::operators;
use super::semi::token_range;
use crate::scan::heredoc::{body_regions_full, line_end_after};
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
    let ops = match operators(line, cond_tokens) {
        Ok(ops) if !ops.is_empty() => ops,
        _ => return Vec::new(),
    };
    let line_end = line_end_after(line, cond_start);
    let regions = match body_regions_full(line, line_end, &ops) {
        Ok((regions, _)) => regions,
        Err(_) => return Vec::new(),
    };
    regions
        .iter()
        .filter_map(|&(s, e)| {
            let bytes = line.get(s..e)?;
            ShortCStr::from_vec(bytes.to_vec()).ok()
        })
        .collect()
}

#[cfg(test)]
mod tests;
