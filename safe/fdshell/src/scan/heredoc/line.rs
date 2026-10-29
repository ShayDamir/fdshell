//! Physical-line heredoc helpers: the line end and the full body regions of a
//! line's `<<` operators (in operator order), so a `;`-terminated operator
//! line still finds its body on the following lines.

use crate::scan::ScanState;

use super::full::{body_regions_full, contents};
use super::operator_delims;
use alloc::vec::Vec;

/// The index just after the first unquoted newline **at or after** `from`
/// (`line.len()` when there is none). `from` is always a physical-line start
/// or a `;`/`\n` boundary (never mid-quote), so a fresh `ScanState` is valid.
pub(crate) fn line_end_after(line: &[u8], from: usize) -> usize {
    let mut state = ScanState::new();
    let mut i = from;
    while i < line.len() {
        let bare =
            !state.in_quote && !state.in_single && !state.in_backtick && state.paren_depth == 0;
        if bare && line.get(i) == Some(&b'\n') {
            return i + 1;
        }
        i = state.advance(line, i);
    }
    line.len()
}

/// The body stream of the physical line `line[line_start..line_end]`: its
/// claimed regions, their bytes, and the resume index — the single allocation
/// point the three extractions below share, so the ranges and the bytes cannot
/// drift apart (one entry per operator keeps the two lists index-aligned with
/// the operator order, LESSONS).
///
/// `line_end` is the command line's end: the newline's position (segment flush)
/// or the index just past it (block-condition extension); the body starts just
/// past that newline in both cases. The resume is the end of the last delimiter
/// line, and the text end when a missing delimiter line claimed the rest of the
/// stream (its bytes are body data, never commands).
///
/// `(empty, empty, line_end)` — the line claims nothing, and the parse layer
/// owns the shape — when it has no operator, when an operator's delimiter word is
/// unreadable, or when the stream starts at/past the text end (no body lines at
/// all: the parse layer accepts an **empty** body and warns, task #364). Every
/// other operator, including one whose delimiter line never arrived, has a region
/// — the first missing one claims the rest of the stream (the `found == 0`
/// exception of task #275 is gone).
fn line_stream_bodies(
    line: &[u8],
    line_start: usize,
    line_end: usize,
) -> (Vec<(usize, usize)>, Vec<Vec<u8>>, usize) {
    let nothing = (Vec::new(), Vec::new(), line_end);
    let Some(ops) = operator_delims(line, line_start, line_end).filter(|ops| !ops.is_empty())
    else {
        return nothing;
    };
    let body_start = if line_end > 0 && line.get(line_end - 1) == Some(&b'\n') {
        line_end
    } else {
        line_end_after(line, line_end)
    };
    if body_start >= line.len() {
        return nothing;
    }
    let alloc = body_regions_full(line, body_start, &ops);
    let resume = alloc.resume;
    let bodies = contents(line, &alloc);
    (alloc.regions, bodies, resume)
}

/// The full body regions (in operator order) of a physical line's `<<`
/// operators, plus the resume index. The regions start just after the line's
/// newline, so a `;`-terminated operator line still finds its body on the
/// following lines.
pub(crate) fn line_bodies_for_line(
    line: &[u8],
    line_start: usize,
    line_end: usize,
) -> (Vec<(usize, usize)>, usize) {
    let (regions, _, resume) = line_stream_bodies(line, line_start, line_end);
    (regions, resume)
}

/// The body **bytes** of a physical line's `<<` operators, in operator order,
/// plus the resume index. The bytes are the region's own text, so a region whose
/// delimiter line never arrived rebuilds a command line the parse layer still
/// decides for — the end-of-file shape, warning included (task #364).
pub(crate) fn line_body_contents(
    line: &[u8],
    line_start: usize,
    line_end: usize,
) -> (Vec<Vec<u8>>, usize) {
    let (_, bodies, resume) = line_stream_bodies(line, line_start, line_end);
    (bodies, resume)
}

/// The regions **and** the bytes the `k` operators written in the window
/// `line[start..]` claim from their physical line's body stream: the line's
/// allocation starting after the `p` operators written on that line **before**
/// the window (the same skip `segment/line.rs::flush_line` applies to a
/// block's trailing statements, and `parse/cond_bodies.rs` to an arm slice).
/// The skip and the take stay one arithmetic unit with the window (LESSONS).
pub(crate) fn claimed_line_bodies(
    line: &[u8],
    line_start: usize,
    start: usize,
    line_end: usize,
    k: usize,
) -> (Vec<(usize, usize)>, Vec<Vec<u8>>) {
    let skip = super::operator_count(line, line_start, start);
    let (regions, bodies, _) = line_stream_bodies(line, line_start, line_end);
    (
        regions.into_iter().skip(skip).take(k).collect(),
        bodies.into_iter().skip(skip).take(k).collect(),
    )
}

/// The start of the physical line that contains `pos` (the index just past the
/// newline preceding it, `0` when there is none). Every whole-line operator
/// scan anchors here: the window and the skip derived from it are one pair
/// (LESSONS), so the three callers that need it share this one rule.
pub(crate) fn line_start(line: &[u8], pos: usize) -> usize {
    line.get(0..pos)
        .and_then(|head| head.iter().rposition(|&b| b == b'\n'))
        .map(|n| n + 1)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
