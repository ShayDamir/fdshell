//! The cond list's *parts*: how a part gets its share of the heredoc bodies,
//! when a part is empty, and how the rest of a failed `&&` chain is skipped up
//! to the next `||`. Body allocation follows the global per-line operator
//! order (LESSONS.md: "A heredoc body is a property of the logical line, not of
//! a contiguous span"); the boundary scanning itself is `boundary`.

use super::boundary;
use crate::scan::{ScanState, heredoc};
use alloc::vec::Vec;

/// After a failing `&&` part, the next `||` (or end of line): everything up
/// to it is skipped without running. The skipped parts' body bytes are still
/// consumed (the bodies are in global operator order).
pub(crate) fn skip_to_or(
    line: &[u8],
    from: usize,
    state: &mut ScanState,
    bodies: &[Vec<u8>],
    body_idx: &mut usize,
) -> usize {
    let mut pos = from;
    loop {
        let (p, is_or) = boundary::next_boundary(line, pos, state);
        take_bodies(line, pos, p, bodies, body_idx);
        if is_or || p == line.len() {
            return p;
        }
        pos = p + 2;
    }
}

/// The next `n` body bytes for the part `line[start..end]`, where `n` is the
/// part's byte-level `<<` operator count. The bodies are in global operator
/// order, so the part's share is the next `n` entries. The count is the
/// **window-blind** one: a part's text is a block/arm text (`cond/part.rs::reconstruct`)
/// and spans body and terminator lines, whose bytes are heredoc structure and
/// never the part's operators.
pub(crate) fn take_bodies(
    line: &[u8],
    start: usize,
    end: usize,
    bodies: &[Vec<u8>],
    body_idx: &mut usize,
) -> Vec<Vec<u8>> {
    let n = heredoc::operator_count_opaque(line, start, end);
    let mut out = Vec::new();
    for _ in 0..n {
        if let Some(body) = bodies.get(*body_idx) {
            out.push(body.clone());
            *body_idx += 1;
        }
    }
    out
}

pub(crate) fn is_empty_part(line: &[u8], start: usize, i: usize) -> bool {
    line.get(start..i).unwrap_or(b"").trim_ascii().is_empty()
}
