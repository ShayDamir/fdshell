use crate::scan::{Boundary, ScanState, advance, boundary};

/// Skip from `i` past a `#` comment to the next `\n` (or end of input).
/// Returns the index to resume scanning from.
pub(crate) fn skip_comment(line: &[u8], mut i: usize) -> usize {
    while i <= line.len() {
        if line.get(i) == Some(&b'\n') {
            i += 1;
            break;
        }
        i += 1;
    }
    i
}

/// Sum the block-depth deltas of the keywords in a space-separated word run.
fn depth_delta(raw: &[u8]) -> i32 {
    raw.split(|&b| b == b' ')
        .filter_map(crate::keywords::keyword_delta)
        .sum()
}

/// Scan forward from `i` looking for the matching closing keyword (depth == 0).
/// Updates `in_quote` and `start` as side effects.
/// Returns `(end_position, block_was_closed)`.
pub(crate) fn scan_block(
    line: &[u8],
    mut i: usize,
    in_quote: &mut bool,
    start: &mut usize,
    mut depth: u32,
) -> (usize, bool) {
    let mut state = ScanState {
        in_quote: *in_quote,
        in_backtick: false,
        dollar_paren_depth: 0,
    };
    while i <= line.len() && depth > 0 {
        let kind = boundary(line, i, &state);
        if kind == Boundary::Char {
            i = advance(line, i, &mut state);
            continue;
        }
        let raw = line.get(*start..i).unwrap_or(b"").trim_ascii();
        depth = depth.saturating_add_signed(depth_delta(raw));
        if kind == Boundary::Comment {
            i = skip_comment(line, i);
            *start = i;
        } else {
            *start = i + 1;
            i += 1;
        }
    }
    *in_quote = state.in_quote;
    (i, depth == 0)
}

#[cfg(test)]
mod tests;
