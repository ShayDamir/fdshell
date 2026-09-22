mod depth_delta;

use crate::scan::{Boundary, ScanState, boundary, heredoc, skip_comment};

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
        word_active: false,
    };
    let mut run_quote = state.in_quote;
    let mut run_backtick = state.in_backtick;
    while i <= line.len() && depth > 0 {
        let kind = boundary(line, i, &state);
        if kind == Boundary::Char {
            i = state.advance(line, i);
            continue;
        }
        // A separator or comment boundary ends the current word.
        state.word_active = false;
        let raw = line.get(*start..i).unwrap_or(b"").trim_ascii();
        depth = depth.saturating_add_signed(depth_delta::depth_delta(raw, run_quote, run_backtick));
        if kind == Boundary::Comment {
            i = skip_comment(line, i);
            *start = i;
        } else if let Some(resume) = heredoc::skip(line, *start, i) {
            // A heredoc body is opaque: its lines carry no block keywords.
            i = resume;
            *start = i + 1;
        } else {
            *start = i + 1;
            i += 1;
        }
        run_quote = state.in_quote;
        run_backtick = state.in_backtick;
    }
    *in_quote = state.in_quote;
    (i, depth == 0)
}

#[cfg(test)]
mod tests;
