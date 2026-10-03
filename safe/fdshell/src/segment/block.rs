use super::Segment;
use crate::comment::scan_block;
use crate::scan::ScanState;
use crate::scan::heredoc;

/// Build the `Segment::Block` for a block-opening keyword at `start`.
/// Returns the segment, the scan resume index, and the resume floor for the
/// rest of the physical line (past which no flush may resume).
/// `pre_resume` is the end of the body regions of a statement that precedes
/// the block on the same line (the line end when there is none).
///
/// The span's exclusive end (`end_pos`) is the closing keyword's separator
/// (`;`/`\n`) position, extended to cover the block's own heredoc bodies when
/// the block text carries `<<` operators (a condition-position heredoc): the
/// bodies sit after the block's command line, in the first body regions of
/// the line, in operator order. A trailing statement's bodies stay outside
/// the span, so a heredoc statement after the block on the same line is not
/// swallowed. The scan resumes just past the closing keyword (a trailing
/// statement on the same line is still scanned) and the floor keeps the
/// line's final flush from re-entering any body region; when the block ends
/// the line the resume is already past all of them.
pub(super) fn keyword_block<'a>(
    line: &'a [u8],
    raw: &[u8],
    part: &[u8],
    start: usize,
    state: &ScanState,
    pre_resume: usize,
) -> (Segment<'a>, usize, usize) {
    let leading_ws = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
    let kw_len = if part.starts_with(b"case") || part.starts_with(b"wait") {
        4
    } else if part.starts_with(b"if") {
        2
    } else if part.starts_with(b"for") {
        3
    } else {
        5
    };
    let after_kw = start + leading_ws + kw_len;
    let mut quote_state = state.in_quote;
    let mut block_start_pos = after_kw;
    let (_end_pos, closed) = scan_block(line, after_kw, &mut quote_state, &mut block_start_pos, 1);
    // The span's exclusive end: the closing keyword's separator (`;`/`\n`)
    // at `block_start_pos - 1` is excluded, or the line end when the closing
    // keyword is the last thing on the input (no separator to step past).
    let mut end = if block_start_pos >= line.len() {
        line.len()
    } else {
        block_start_pos - 1
    };
    let (resume, floor) = if closed {
        let line_end = heredoc::line_end_after(line, start);
        let (bodies, line_resume) = heredoc::line_bodies_for_line(line, start, line_end);
        // The block text's operators (condition and body) claim the line's
        // body regions in operator order; a trailing statement's operators
        // come after them and keep their regions outside the span.
        let k = heredoc::operator_count(line, start, block_start_pos);
        if k > 0 {
            // Extend forward only: a body region may end before the closing
            // keyword (a multi-line block with a heredoc in its body), and
            // the span must keep covering the whole block text.
            end = match bodies.get(k - 1) {
                Some(&(_, e)) => end.max(e),
                None => end.max(line_resume),
            };
        }
        if block_start_pos < line_end {
            // Trailing content on the same line: scan it; the line's final
            // flush must not re-enter the body regions.
            (block_start_pos, line_resume.max(pre_resume))
        } else {
            // The block ends the line (or continues on later lines): resume
            // just past the closing keyword, never inside a body region.
            (block_start_pos.max(line_resume).max(pre_resume), 0)
        }
    } else {
        (end + 1, 0)
    };
    (
        Segment::Block {
            block_start: start,
            end_pos: end,
            closed,
        },
        resume,
        floor,
    )
}
