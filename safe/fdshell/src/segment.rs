mod block;
mod line;

use crate::brace::scan_function_block;
use crate::keywords::keyword_delta;
use crate::scan::{Boundary, ScanState, boundary, skip_comment};
use alloc::vec::Vec;

/// A segment of a script line extracted by the scanner.
pub(crate) enum Segment<'a> {
    /// Simple statement: the command bytes (`cmd`), its offset in the line
    /// (`off`), and its heredoc body regions (`bodies`, empty when the
    /// statement has no heredoc).
    Statement {
        cmd: &'a [u8],
        off: usize,
        bodies: Vec<(usize, usize)>,
    },
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
    let mut line_start = 0;
    let mut runs: Vec<(&[u8], usize)> = Vec::new();

    while i <= line.len() {
        let kind = boundary(line, i, &state);
        if kind == Boundary::Char {
            i = state.advance(line, i);
            continue;
        }
        state.word_active = false;

        let raw = line.get(start..i).unwrap_or(b"");
        let part = raw.trim_ascii();

        if !in_block && !part.is_empty() && keyword_delta(part) == Some(1) {
            line::flush_line(&mut segments, line, line_start, i, &mut runs);
            let (segment, end) = block::keyword_block(line, raw, part, start, &state);
            segments.push(segment);
            i = end + 1;
            start = i;
            line_start = i;
        } else if !in_block
            && let Some((end, closed)) = scan_function_block(line, part, start, state.in_quote)
        {
            line::flush_line(&mut segments, line, line_start, i, &mut runs);
            segments.push(Segment::Block {
                block_start: start,
                end_pos: end,
                closed,
            });
            i = end + 1;
            start = i;
            line_start = i;
        } else {
            if !part.is_empty() {
                let lead = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
                runs.push((part, start + lead));
            }
            if line.get(i) == Some(&b'\n') || i == line.len() {
                let resume = line::flush_line(&mut segments, line, line_start, i, &mut runs);
                i = resume;
                line_start = i;
            }
            if kind == Boundary::Comment {
                i = skip_comment(line, i);
                start = i;
                // The comment ends the physical line: the next line starts at
                // `i`, so the buffered runs' line bookkeeping restarts there.
                line_start = i;
            } else {
                start = i + 1;
                i += 1;
            }
        }
    }
    segments
}

#[cfg(test)]
mod tests;
