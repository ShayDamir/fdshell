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
    let mut dollar_paren_depth = 0u32;
    let mut in_backtick = false;
    let mut i = 0;

    while i <= line.len() {
        let is_comment = !in_quote && !in_backtick && line.get(i) == Some(&b'#');
        let is_sep = i == line.len()
            || (!in_quote
                && !in_backtick
                && dollar_paren_depth == 0
                && matches!(line.get(i), Some(&b';') | Some(&b'\n')));

        if is_comment || is_sep {
            let part = line.get(start..i).unwrap_or(b"").trim_ascii();

            if !in_block && !part.is_empty() && keyword_delta(part) == Some(1) {
                let block_start = start;
                let original = line.get(block_start..i).unwrap_or(b"");
                let leading_ws = original
                    .iter()
                    .take_while(|&&b| b.is_ascii_whitespace())
                    .count();
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

            if is_comment {
                i = skip_comment(line, i);
                start = i;
                continue;
            } else {
                start = i + 1;
            }
        } else if line.get(i) == Some(&b'"') {
            in_quote = !in_quote;
        } else if !in_quote && !in_backtick && line.get(i) == Some(&b'$') {
            if line.get(i + 1) == Some(&b'(') {
                dollar_paren_depth = dollar_paren_depth.saturating_add(1);
                i += 1;
            }
        } else if !in_quote && !in_backtick && line.get(i) == Some(&b'(') {
            if dollar_paren_depth > 0 {
                dollar_paren_depth = dollar_paren_depth.saturating_add(1);
            }
        } else if !in_quote && !in_backtick && line.get(i) == Some(&b')') {
            dollar_paren_depth = dollar_paren_depth.saturating_sub(1);
        } else if !in_quote && !in_backtick && line.get(i) == Some(&b'`') {
            in_backtick = true;
        } else if in_backtick && line.get(i) == Some(&b'`') {
            in_backtick = false;
        }

        i += 1;
    }
    segments
}

#[cfg(test)]
mod tests;
