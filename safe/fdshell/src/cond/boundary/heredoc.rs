//! The heredoc **body regions** of a cond-list run: the region walk that
//! keeps a statement's bodies opaque while an operator written inside the
//! body region of a keyword block's own text (a block condition's bodies
//! travel inside the span) stays a split point. Regions come from the one
//! shared line helper, so a `;`-terminated operator line finds its bodies on
//! the following lines exactly as every other layer does.

use crate::scan::ScanState;
use crate::scan::heredoc::line_bodies_for_line;
use alloc::vec::Vec;

/// The body regions of the run that ends at the newline `nl`: the regions of
/// the **physical line** the run sits on (a `;`-terminated operator line
/// `if cat <<X; then` finds its bodies on the following lines), keeping only
/// the regions whose bytes are inside the scanned text. A statement's bodies
/// travel outside its text, so its regions start at the text end and drop
/// out; a block that extends its text over its bodies keeps them.
/// Plus the resume index: the newline terminating the last delimiter line.
pub(super) fn regions(line: &[u8], run_start: usize, nl: usize) -> (Vec<(usize, usize)>, usize) {
    let (regions, resume) = line_bodies_for_line(line, line_start_of(line, run_start), nl + 1);
    let kept: Vec<(usize, usize)> = regions
        .into_iter()
        .filter(|&(start, _)| start < line.len())
        .collect();
    (kept, resume)
}

/// The start of the physical line containing `run_start` (the index just
/// past the newline that precedes it, `0` when there is none).
fn line_start_of(line: &[u8], run_start: usize) -> usize {
    line.get(0..run_start)
        .and_then(|s| s.iter().rposition(|&b| b == b'\n'))
        .map(|p| p + 1)
        .unwrap_or(0)
}

/// Walk the regions of one heredoc-carrying run: inside a keyword block
/// (`block_depth > 0`) a region is scanned for its earliest top-level `&&` /
/// `||` — the pinned body-`&&` split
/// (`tests/heredoc.rs::block_heredoc_cond_run_and_and_in_body_splits_part`) —
/// and elsewhere it is skipped whole. `i` ends at the last skipped region's
/// newline; `Some(op)` is the boundary to return.
pub(super) fn walk(
    regions: &[(usize, usize)],
    block_depth: i32,
    line: &[u8],
    i: &mut usize,
) -> Option<(usize, bool)> {
    for &(start, end) in regions {
        if block_depth > 0
            && let Some(op) = operator_in_region(line, start, end)
        {
            return Some(op);
        }
        // A region's own newline is not consumed: the caller lands on it and
        // the plain `\n` arm resets the run, exactly as a plain line's does.
        *i = end - 1;
    }
    None
}

/// The earliest top-level `&&` / `||` of the region `line[start..end]`, with
/// the region scanned in a **fresh** state: a body legitimately carries
/// unbalanced quotes, backticks and `$( … )`, and its bytes must never fold
/// into the caller's quote or substitution state.
fn operator_in_region(line: &[u8], start: usize, end: usize) -> Option<(usize, bool)> {
    let mut state = ScanState::new();
    let mut i = start;
    while i < end {
        let b = line.get(i).copied().unwrap_or(0);
        let top =
            !state.in_quote && !state.in_single && !state.in_backtick && state.paren_depth == 0;
        let next = line.get(i + 1).copied().unwrap_or(0);
        if top && b == next && matches!(b, b'&' | b'|') {
            return Some((i, b == b'|'));
        }
        i = state.advance(line, i);
    }
    None
}

#[cfg(test)]
mod tests;
