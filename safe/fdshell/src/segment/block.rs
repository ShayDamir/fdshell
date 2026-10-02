use super::Segment;
use crate::comment::scan_block;
use crate::scan::ScanState;
use crate::scan::heredoc;

/// Build the `Segment::Block` for a block-opening keyword at `start`.
/// Returns the segment and the position of the last byte of the block.
pub(super) fn keyword_block<'a>(
    line: &'a [u8],
    raw: &[u8],
    part: &[u8],
    start: usize,
    state: &ScanState,
) -> (Segment<'a>, usize) {
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
    // Extend the block span to cover heredoc bodies that follow the closing
    // keyword (a condition-position heredoc: the bodies are part of the
    // block's condition, after the block's command line). A body-position
    // heredoc's region is inside the block (the closing keyword follows it),
    // so the block ends at its closing keyword as before. The condition line
    // is the block's own physical line (starting at `start`), so the bodies
    // start just past that line's newline; `end` is the span's exclusive end
    // (the newline terminating the last delimiter line).
    if closed {
        let line_end = heredoc::line_end_after(line, start);
        if line_end < line.len() {
            let (bodies, resume) = heredoc::line_bodies_for_line(line, start, line_end);
            if bodies.first().is_some_and(|&(s, _)| s >= block_start_pos) {
                end = resume;
            }
        }
    }
    (
        Segment::Block {
            block_start: start,
            end_pos: end,
            closed,
        },
        end,
    )
}
