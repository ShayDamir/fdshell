use crate::comment::{scan_block, skip_comment};
use crate::keywords::keyword_delta;
use crate::scan::{Boundary, ScanState, advance, boundary};
use alloc::vec::Vec;

/// A segment of a script line extracted by the scanner.
pub(crate) enum Segment<'a> {
    /// Simple statement ending at a separator (`;` or `\n`).
    /// The second element is the offset of the first byte within the line.
    Statement(&'a [u8], usize),
    /// Block (e.g. `if … fi`) spanning from `block_start` to `end_pos`.
    Block {
        block_start: usize,
        end_pos: usize,
        /// Whether the closing keyword was found.
        closed: bool,
    },
}

/// Scan a script line and return segments with their positions.
///
/// When `in_block` is true, block-opening keywords (if/for/while/case) are
/// treated as regular statement content rather than new blocks. This prevents
/// re-detecting nested blocks inside already-scanned block bodies.
pub(crate) fn scan_segments(line: &[u8], in_block: bool) -> Vec<Segment<'_>> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut state = ScanState::new();
    let mut i = 0;

    while i <= line.len() {
        let kind = boundary(line, i, &state);
        if kind == Boundary::Char {
            i = advance(line, i, &mut state);
            continue;
        }
        // A separator or comment boundary ends the current word.
        state.word_active = false;

        let raw = line.get(start..i).unwrap_or(b"");
        let part = raw.trim_ascii();

        if !in_block && !part.is_empty() && keyword_delta(part) == Some(1) {
            let block_start = start;
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
            let after_kw = block_start + leading_ws + kw_len;
            let mut quote_state = state.in_quote;
            let mut block_start_pos = after_kw;
            let (_end_pos, closed) =
                scan_block(line, after_kw, &mut quote_state, &mut block_start_pos, 1);

            let end = line.len().min(block_start_pos.saturating_sub(1));
            segments.push(Segment::Block {
                block_start,
                end_pos: end,
                closed,
            });
            i = end;
        } else if !part.is_empty() {
            let lead = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
            segments.push(Segment::Statement(part, start + lead));
        }

        if kind == Boundary::Comment {
            i = skip_comment(line, i);
            start = i;
        } else {
            start = i + 1;
            i += 1;
        }
    }
    segments
}

#[cfg(test)]
mod tests;
