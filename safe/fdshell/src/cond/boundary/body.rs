//! The heredoc **body regions** of a cond-list run: the region step that keeps
//! a statement's bodies opaque while an operator written inside the body region
//! of a keyword block's own text (a block condition's bodies travel inside the
//! span) stays a split point. Regions come from the one shared line helper, so
//! a `;`-terminated operator line finds its bodies on the following lines
//! exactly as every other layer does.

use crate::scan::ScanState;
use crate::scan::heredoc::line_bodies_for_line;
use alloc::vec::Vec;

/// What the region walk makes of a run that ends at a newline.
#[cfg_attr(test, derive(Debug, PartialEq))]
pub(super) enum Step {
    /// A block's own body region carries a top-level `&&` / `||`: the byte of
    /// its first one is the cond-list boundary (second field: it is `||`).
    Split(usize, bool),
    /// Every region of the run is opaque: continue at the newline terminating
    /// the last delimiter line.
    Skip(usize),
}

/// The cond boundary's whole heredoc rule, in one place: take the body regions
/// of the run that ends at the newline `nl` and walk them. Inside a keyword
/// block (`block_depth > 0`) each region is scanned for its earliest top-level
/// `&&` / `||` — the pinned body-`&&` split
/// (`tests/heredoc.rs::block_heredoc_cond_run_and_and_in_body_splits_part`);
/// a statement's regions are skipped whole. `None` when the run carries no
/// regions at all, so the caller's plain `\n` arm applies.
pub(super) fn step(line: &[u8], run_start: usize, nl: usize, block_depth: i32) -> Option<Step> {
    let (regions, resume) = regions(line, run_start, nl);
    if regions.is_empty() {
        return None;
    }
    if block_depth > 0 {
        for &(start, end) in &regions {
            if let Some((at, is_or)) = operator_in_region(line, start, end) {
                return Some(Step::Split(at, is_or));
            }
        }
    }
    Some(Step::Skip(resume))
}

/// The body regions of the run ending at the newline `nl`, plus the resume
/// index (the newline terminating the last delimiter line): the regions of the
/// **physical line** the run sits on, so a `;`-terminated operator line
/// `if cat <<X; then` finds its bodies on the following lines. A run whose
/// bodies travel outside the scanned text (a top-level statement's part is its
/// command bytes; the bodies are `run_cond_list`'s own `bodies`) gets none: the
/// helper starts a region past the text end and returns an empty list.
fn regions(line: &[u8], run_start: usize, nl: usize) -> (Vec<(usize, usize)>, usize) {
    line_bodies_for_line(line, line_start_of(line, run_start), nl + 1)
}

/// The start of the physical line containing `run_start` (the index just
/// past the newline that precedes it, `0` when there is none).
fn line_start_of(line: &[u8], run_start: usize) -> usize {
    line.get(0..run_start)
        .and_then(|s| s.iter().rposition(|&b| b == b'\n'))
        .map(|p| p + 1)
        .unwrap_or(0)
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
