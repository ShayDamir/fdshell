use crate::comment::{scan_block, skip_comment};
use crate::keywords::keyword_delta;
use alloc::vec::Vec;

/// A segment of a script line extracted by the scanner.
pub(crate) enum Segment<'a> {
    /// Simple statement ending at a separator (`;` or `\n`).
    Statement(&'a [u8]),
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
    let mut in_quote = false;
    let mut i = 0;

    while i <= line.len() {
        if !in_quote && line.get(i) == Some(&b'#') {
            i = skip_comment(line, i);
            start = i;
            continue;
        }

        if line.get(i) == Some(&b'"') {
            in_quote = !in_quote;
        } else if i == line.len()
            || (!in_quote && matches!(line.get(i), Some(&b';') | Some(&b'\n')))
        {
            let part = line.get(start..i).unwrap_or(b"").trim_ascii();

            if !in_block && !part.is_empty() && keyword_delta(part) == Some(1) {
                let block_start = start;
                let original = line.get(block_start..i).unwrap_or(b"");
                let leading_ws = original
                    .iter()
                    .take_while(|&&b| b.is_ascii_whitespace())
                    .count();
                let kw_len = match part {
                    p if p.starts_with(b"case") => 4,
                    p if p.starts_with(b"if") => 2,
                    p if p.starts_with(b"for") => 3,
                    _ => 5,
                };
                let after_kw = block_start + leading_ws + kw_len;
                let mut quote_state = in_quote;
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
                segments.push(Segment::Statement(part));
            }

            start = i + 1;
        }
        i += 1;
    }
    segments
}
