use super::Segment;
pub(super) mod keyword;
use crate::brace::scan_function_block;
use crate::scan::ScanState;
use crate::scan::heredoc;

/// Build the `Segment::Block` for a block-opening word at `start`, and the
/// scan bookkeeping for the rest of the physical line: the resume index, the
/// resume floor (past which no flush may resume), and the line start the
/// trailing runs' bookkeeping continues from.
///
/// Dispatch order: a `name() { … }` function body, then a keyword block.
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
) -> (Segment<'a>, usize, usize, usize) {
    if let Some((end, closed)) = scan_function_block(line, part, start, state.in_quote) {
        (
            Segment::Block {
                block_start: start,
                end_pos: end,
                closed,
            },
            end + 1,
            pre_resume,
            end + 1,
        )
    } else {
        let (segment, resume, floor) =
            keyword::keyword_block(line, raw, part, start, state, pre_resume);
        // Trailing content shares the block's physical line: the line's
        // bookkeeping (and its global body stream) continues from the line's
        // start, so the trailing runs' flush skips the regions the block's
        // operators claimed.
        let new_start = if resume < heredoc::line_end_after(line, start) {
            line_start
        } else {
            resume
        };
        (segment, resume, floor, new_start)
    }
}
