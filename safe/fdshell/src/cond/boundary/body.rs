//! The heredoc **body regions** of a cond-list run: the region step that keeps
//! a statement's bodies opaque while an operator written inside the body region
//! of a keyword block's own text (a block condition's bodies travel inside the
//! span) stays a split point. The scan is **position-gated**: a region is
//! scanned only where the `<<` that claimed it sits in the block's *clause*
//! text, and a region claimed by a word of the block's **body** is the body's
//! own data ([`crate::cond::boundary::block::Modes`], task #304). Regions come
//! from the one shared line helper, so a `;`-terminated operator line finds its
//! bodies on the following lines exactly as every other layer does.

use crate::scan::ScanState;
use crate::scan::heredoc::{line_bodies_for_line, line_start, operator_sites};

use super::block::{Mode, Modes};

/// What the region walk makes of a run that ends at a newline.
#[cfg_attr(test, derive(Debug, PartialEq))]
pub(super) enum Step {
    /// A block's **clause** carries a `<<` whose body region holds a top-level
    /// `&&` / `||`: the byte of its first one is the cond-list boundary
    /// (second field: it is `||`).
    Split(usize, bool),
    /// Every region of the run is opaque: continue at the newline terminating
    /// the last delimiter line.
    Skip(usize),
}

/// The cond boundary's whole heredoc rule, in one place: take the body regions
/// of the run that ends at the newline `nl` and walk them. Inside a keyword
/// block (the `in_block` half of its pairing, task #309) each region is scanned
/// for its earliest top-level
/// `&&` / `||` — the pinned clause-`&&` split
/// (`tests/heredoc.rs::block_heredoc_cond_run_and_and_in_body_splits_part`) —
/// **only where its `<<` sits in the block's clause**: a region claimed by a
/// word of the block's body is skipped whole
/// (`tests/heredoc.rs::and_in_a_body_position_heredoc_region_is_body_data`).
/// A statement's regions are skipped whole either way. `None` when the run
/// carries no regions at all, so the caller's plain `\n` arm applies.
pub(super) fn step(
    line: &[u8],
    run_start: usize,
    nl: usize,
    in_block: bool,
    modes: &Modes,
) -> Option<Step> {
    // The regions of the run's **physical line** and their `<<` sites, both
    // taken from the one window: the k-th region belongs to the k-th site,
    // because the allocation is one region per operator (`scan/heredoc/full.rs`).
    let (from, to) = window(line, run_start, nl);
    let (regions, resume) = line_bodies_for_line(line, from, to);
    if regions.is_empty() {
        return None;
    }
    if in_block {
        for (&(start, end), site) in regions.iter().zip(operator_sites(line, from, to)) {
            if let Some((at, is_or)) = scanned_operator(line, (start, end), site, modes) {
                return Some(Step::Split(at, is_or));
            }
        }
    }
    Some(Step::Skip(resume))
}

/// The window the run ending at the newline `nl` is scanned from: the regions
/// of the **physical line** the run sits on, so a `;`-terminated operator line
/// `if cat <<X; then` finds its bodies on the following lines. A run whose
/// bodies travel outside the scanned text (a top-level statement's part is its
/// command bytes; the bodies are `run_cond_list`'s own `bodies`) gets none: the
/// helper starts a region past the text end and returns an empty list. Never
/// widen it: a rule fixed at one granularity is a divergence at the other.
fn window(line: &[u8], run_start: usize, nl: usize) -> (usize, usize) {
    (line_start(line, run_start), nl + 1)
}

/// The earliest top-level operator of the region `(start, end)` — the region
/// claimed by the `<<` at `site` — but only when that site sits in the block's
/// **clause** text. A site in the block's body claims a region that is the
/// body's own data: opaque, no split point.
fn scanned_operator(
    line: &[u8],
    (start, end): (usize, usize),
    site: usize,
    modes: &Modes,
) -> Option<(usize, bool)> {
    if matches!(modes.mode_at(site), Mode::Body) {
        return None;
    }
    operator_in_region(line, start, end)
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
