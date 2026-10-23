//! The regions a block's own command text claims from its physical line's body
//! stream, shared by the two block branches (the keyword block and the function
//! definition): one append/extension rule, so the two branches can never drift.

use crate::scan::heredoc;
use alloc::vec::Vec;

/// The body bytes the `<<` operators written in the block text
/// `line[start..end]` claim from their physical line's body stream, plus the
/// span end they imply:
///
/// **append mode** — the first claimed region starts at or past `end`, so the
/// regions sit **outside** the block's text: the result is the region contents
/// and `end` unchanged, and the caller delivers them as the block's bodies
/// (`script.rs`'s block arm hands them to `run_cond_list`, task #234), so the
/// part that owns a `<<` operator is the one that gets its body.
///
/// **extension mode** — the regions are interleaved with the block's own lines
/// (a multi-line block, a multi-line function body): the result carries no
/// contents and the span end **extended forward** to the block's last claimed
/// region, never past it, so the span keeps covering the whole block text.
///
/// `(empty, end)` — the block claims nothing — when its text has no `<<`
/// operator, when the block is **unclosed** (the caller reports the missing
/// closer), or when the line's stream claims nothing at all (its **first**
/// delimiter line is missing, task #275's rule).
///
/// `line_start` is the start of the physical line the block opens on: the skip
/// of the operators written on that line **before** the block and the whole-line
/// stream are one pair with the window (LESSONS). `line_resume` is the end of
/// the line's last body region (the line end when the bodies are missing), the
/// fallback the extension mode uses when the window claims no region.
pub(super) fn claimed_bodies(
    line: &[u8],
    line_start: usize,
    start: usize,
    mut end: usize,
    line_end: usize,
    line_resume: usize,
    closed: bool,
) -> (Vec<Vec<u8>>, usize) {
    let nothing = (Vec::new(), end);
    if !closed {
        return nothing;
    }
    // The count is the window-blind one: a block text spans body and terminator
    // lines, whose bytes are heredoc structure, never the block's operators.
    // Regions claimed by an operator written **before** the window stay visible
    // — blinding them would hide the block's own closer.
    let k = heredoc::operator_count_opaque(line, start, end);
    if k == 0 {
        return nothing;
    }
    let (claimed, contents) = heredoc::claimed_line_bodies(line, line_start, start, line_end, k);
    if claimed.first().is_some_and(|&(s, _)| s >= end) {
        return (contents, end);
    }
    // Extension mode: forward-only — the span keeps the block text.
    end = match claimed.last() {
        Some(&(_, e)) => end.max(e),
        None => end.max(line_resume),
    };
    (Vec::new(), end)
}

#[cfg(test)]
mod tests;
