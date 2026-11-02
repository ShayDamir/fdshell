//! The rules for a window that **spans several physical lines**: where its body
//! stream starts, and what a run of that size attaches. The single-line rules stay
//! in the parent; a rule fixed at one granularity is a divergence at the other
//! (LESSONS), so the two granularities sit side by side and cannot drift.

use alloc::vec::Vec;

use crate::scan::heredoc::full::{Allocation, contents};
use crate::scan::heredoc::ops::operator_sites;
use crate::scan::heredoc::regions::{body_regions, terminator_end};

use super::{line_end_after, line_start};

/// Where the window's body stream starts: just past the newline ending the
/// physical line that carries the window's **first** operator. A window that stops
/// on that line keeps its own end — every single-line window lands there, so the
/// `single_line` rule of `segment/line.rs::flush_line` (tasks #232/#295) and the
/// window+skip pair of task #138 stay byte-identical — while a window that runs
/// past it starts its stream after the first operator's line, so each physical line
/// inside the run owns exactly one body stream and [`body_regions_full`] hands its
/// regions out in global per-line operator order. A rule fixed at one granularity is
/// a divergence at the other (LESSONS).
///
/// [`body_regions_full`]: crate::scan::heredoc::full::body_regions_full
pub(crate) fn stream_start(line: &[u8], window_start: usize, line_end: usize) -> usize {
    match operator_sites(line, window_start, line_end).first() {
        Some(&op) if line_start(line, op) < line_start(line, line_end) => {
            next_line_start(line, line_start(line, op))
        }
        _ if line_end > 0 && line.get(line_end - 1) == Some(&b'\n') => line_end,
        _ => line_end_after(line, line_end),
    }
}

/// The start of the next physical line at/after `from`: a plain newline scan, the
/// text end when there is none. A physical line's end is its terminating newline —
/// for a window that stops on the line the window's own edge is the same boundary,
/// so the plain scan is that rule at the granularity of a window spanning lines.
fn next_line_start(line: &[u8], from: usize) -> usize {
    line.get(from..)
        .and_then(|s| s.iter().position(|&b| b == b'\n'))
        .map_or(line.len(), |p| from + p + 1)
}

/// The body bytes a run whose **own bytes span several physical lines** attaches:
/// the regions the single definition of an opaque body ([`body_regions`], the same
/// scan the tokenizer blanks with) assigns to bytes **past** the run's end —
/// append mode, the execution layer re-attaches those. A region that falls inside
/// the run's bytes is **extension mode**: its bytes already sit in the run's text,
/// so nothing is attached and the parse layer re-derives the allocation from the
/// text. Inside the run, every physical line owns exactly one body stream and hands
/// its regions out in operator order, so the allocations come out in global per-line
/// operator order — the run's closer search already skips those regions (the same
/// single definition), which is why a construct whose closer sits on a region line
/// reports its own shape (task #403). One append/extension rule with
/// `segment/block/claim.rs`, so the branches cannot drift; a run of one physical
/// line keeps the flush's own allocation (tasks #232/#295).
///
/// [`body_regions`]: crate::scan::heredoc::regions::body_regions
pub(crate) fn run_line_bodies(line: &[u8], run_end: usize) -> Vec<Vec<u8>> {
    let regions: Vec<(usize, usize)> = body_regions(line)
        .into_iter()
        .filter(|&(body_start, _)| body_start >= run_end)
        .map(|(body_start, term_start)| (body_start, terminator_end(line, term_start)))
        .collect();
    let found = regions.len();
    contents(
        line,
        &Allocation {
            regions,
            resume: 0,
            found,
        },
    )
}

#[cfg(test)]
mod tests;
