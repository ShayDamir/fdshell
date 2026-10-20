//! The window-blind operator count: a heredoc's body region and its terminator
//! line are heredoc structure, so a run window that spans them counts only the
//! operators written outside the regions. `sites` is reused unchanged, so the
//! recorded-equivalent mutants of `ops.rs` stay valid: this module exists for
//! the count that must ignore claimed regions, never as a second walker.

use super::line::line_start;
use super::ops::sites;
use super::regions::{body_regions, terminator_end};
use alloc::vec::Vec;

/// The number of `<<` operators in the run `line[from..to]`, blind to the
/// text's opaque heredoc spans (body regions and their terminator lines): a
/// terminator line's own `<<` is heredoc data, never an operator, so a window
/// that spans a claimed region counts only the operators written outside the
/// regions. The token level sees the same shape because `tokenize_statement`
/// blanks a terminator line's structure bytes; the two counts must agree
/// (LESSONS: the byte-level and the token-level rule must agree).
///
/// Only regions **claimed inside the window** are skipped: a region claimed by
/// an operator written before `from` belongs to that earlier statement, and its
/// body plus terminator line cover the window's own structure — subtracting
/// them would blind the count to a block closer written behind them
/// (`segment/block/keyword/tests.rs::skip_is_bounded_to_the_blocks_physical_line`).
/// Call sites whose window is bounded to one physical line need no variant:
/// the window-subset invariant of `regions.rs` keeps a claimed region out of
/// them (`scan/heredoc/regions.rs`).
pub(crate) fn operator_count_opaque(line: &[u8], from: usize, to: usize) -> usize {
    let mut spans: Vec<(usize, usize)> = body_regions(line)
        .into_iter()
        .filter(|(start, _)| line_start(line, start.saturating_sub(1)) >= from)
        .map(|(start, term_start)| (start, terminator_end(line, term_start)))
        .collect();
    spans.sort_unstable();
    let mut found = 0usize;
    let mut pos = from;
    for (start, end) in spans {
        if end <= pos {
            continue;
        }
        if start >= to {
            break;
        }
        found += sites(line, pos, start.min(to)).len();
        pos = end;
    }
    found + sites(line, pos, to).len()
}

