use super::Segment;
use crate::comment::scan_block;
use crate::scan::ScanState;

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
    let kw_len = if part.starts_with(b"case") {
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
    let end = line.len().min(block_start_pos.saturating_sub(1));
    (
        Segment::Block {
            block_start: start,
            end_pos: end,
            closed,
        },
        end,
    )
}
