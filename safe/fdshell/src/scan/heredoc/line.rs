//! Physical-line heredoc helpers: the line end and the full body regions of a
//! line's `<<` operators (in operator order), so a `;`-terminated operator
//! line still finds its body on the following lines.

use crate::scan::ScanState;

use super::full::body_regions_full;
use super::operator_delims;
use alloc::vec::Vec;

/// The index just after the first unquoted newline **at or after** `from`
/// (`line.len()` when there is none). `from` is always a physical-line start
/// or a `;`/`\n` boundary (never mid-quote), so a fresh `ScanState` is valid.
pub(crate) fn line_end_after(line: &[u8], from: usize) -> usize {
    let mut state = ScanState::new();
    let mut i = from;
    while i < line.len() {
        let bare = !state.in_quote && !state.in_backtick && state.paren_depth == 0;
        if bare && line.get(i) == Some(&b'\n') {
            return i + 1;
        }
        i = state.advance(line, i);
    }
    line.len()
}

/// The full body regions (in operator order) of a physical line's `<<`
/// operators, plus the resume index (end of the last delimiter line). The
/// regions start just after the line's newline, so a `;`-terminated operator
/// line still finds its body on the following lines. `(empty, line_end)` when
/// the line has no operators or the body is missing.
///
/// `line_end` is the command line's end: the newline's position (segment
/// flush) or the index just past it (block-condition extension); the body
/// starts just past that newline in both cases.
pub(crate) fn line_bodies_for_line(
    line: &[u8],
    line_start: usize,
    line_end: usize,
) -> (Vec<(usize, usize)>, usize) {
    let ops = match operator_delims(line, line_start, line_end) {
        Some(ops) if !ops.is_empty() => ops,
        _ => return (Vec::new(), line_end),
    };
    let body_start = if line_end > 0 && line.get(line_end - 1) == Some(&b'\n') {
        line_end
    } else {
        line_end_after(line, line_end)
    };
    if body_start >= line.len() {
        return (Vec::new(), line_end);
    }
    match body_regions_full(line, body_start, &ops) {
        Ok((regions, resume)) => (regions, resume),
        Err(_) => (Vec::new(), line_end),
    }
}

/// The number of `<<` operators in the run `line[from..to]` (0 when a bare
/// `<<` has no delimiter word). The byte-level count the execution layer uses
/// to assign a line's body regions to its parts.
pub(crate) fn operator_count(line: &[u8], from: usize, to: usize) -> usize {
    operator_delims(line, from, to)
        .map(|ops| ops.len())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
