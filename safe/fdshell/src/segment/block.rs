// mutants: -C --lib -C --test -C function_keyword -C --test -C block_bodies -C --test -C block_closers -C --test -C block_conditions -- -E 'test(/^segment::/) | binary(function_keyword) | binary(block_bodies) | binary(block_closers) | binary(block_conditions)'

use super::Segment;
mod builder;
pub(super) mod claim;
pub(super) mod keyword;
mod length;
pub(super) mod midrun;
pub(super) mod opener;
pub(super) mod stage;
use crate::brace::{scan_arith_for_block, scan_function_block};
use crate::keywords::leading_ws;
use crate::scan::ScanState;
use crate::scan::heredoc;
use crate::segment::book::Book;
use crate::segment::line;
use core::cmp::Ordering;

/// Take the keyword-block branch of the segment scan for the block-opening run
/// `raw` that starts at `book.start`: the run's statements are flushed, then
/// the block segment is pushed, and the cursor is rewound past the block's span
/// with the line start the trailing runs continue from and the resume floor for
/// the rest of the physical line.
///
/// The gate that picks this branch is the shared [`opener`] rule: the separator
/// arm and the plain-`&` arm read it identically (task #354).
pub(super) fn open<'a>(line: &'a [u8], raw: &[u8], book: &mut Book<'a>) {
    let (start, line_start) = (book.start, book.line_start);
    let part = raw.trim_ascii();
    let pre_resume = line::flush::flush_line(book, line, line_start, start + raw.len());
    let (segment, resume, floor, new_start, stray) =
        open_block(line, raw, part, start, &book.state, pre_resume, line_start);
    book.segments.push(segment);
    // The span walk's decision (task #436): the scan marks the line, and
    // `script.rs`'s preflight bails on it before anything runs. The marker rides
    // beside the segment the walk belonged to.
    if let Some(off) = stray {
        book.segments.push(Segment::StrayCloser { off });
    }
    // `open_block`'s line start is the resume itself exactly when the block
    // ends the physical line, which is the case [`Book::rewind`] floors.
    book.rewind(resume, new_start, floor, new_start == resume);
}

/// Build the `Segment::Block` for a block-opening word at `start`, and the
/// scan bookkeeping for the rest of the physical line: the resume index, the
/// resume floor (past which no flush may resume), and the line start the
/// trailing runs' bookkeeping continues from.
///
/// Dispatch order: a `name() { … }` function body or an arithmetic-`for` head
/// (task #394) — both are head words whose `{ …; }` body the span swallows, so both
/// go to [`builder::function_block`] — and then a keyword block.
/// Each branch is guarded by the same predicate the segment scanner tests, so
/// the scanner's condition and this dispatch agree. The `{ …; }` group is not
/// dispatched here: the scanner recognises its opener at the byte level (a
/// group at a word start, mid-line), so a group run never reaches this
/// separator path.
/// `pre_resume` is the end of the body regions of a statement that precedes
/// the block on the same line (the line end when there is none); `line_start`
/// is the start of the physical line the scanner is flushing.
pub(super) fn open_block<'a>(
    line: &'a [u8],
    raw: &[u8],
    part: &[u8],
    start: usize,
    state: &ScanState,
    pre_resume: usize,
    line_start: usize,
) -> (Segment<'a>, usize, usize, usize, Option<usize>) {
    // The definer gate walks `line` from `part`'s own first byte, not the run's.
    let def_start = start + leading_ws(raw);
    let span = scan_function_block(line, part, def_start, state.quotes())
        .or_else(|| scan_arith_for_block(line, part, def_start, state.quotes()));
    let (segment, resume, floor, stray) = match span {
        // The definer and the arithmetic-`for` head (task #394) share the block
        // builder: both are head words whose `{ …; }` body the span swallows.
        Some((end, closed)) => {
            builder::function_block(line, start, end, closed, pre_resume, line_start)
        }
        None => keyword::keyword_block(line, raw, part, start, line_start, state, pre_resume),
    };
    // Trailing content shares the block's physical line: the line's bookkeeping
    // (and its global body stream) continues from the line's start, so the
    // trailing runs' flush skips the regions the pre-block statements and the
    // block's own operators claimed. The test is written operator-free on
    // purpose: at equality (`resume == line_end`, the definition's closing token
    // the physical line's last byte) both branches already agree — the equality
    // case's resume was pushed past every body region of the line, so the
    // remaining scan is a flush of an empty run buffer either way — and a
    // comparison here would carry a family no test can see.
    let line_end = heredoc::line_end_after(line, start);
    let new_start = if matches!(resume.cmp(&line_end), Ordering::Less) {
        line_start
    } else {
        resume
    };
    (segment, resume, floor, new_start, stray)
}
