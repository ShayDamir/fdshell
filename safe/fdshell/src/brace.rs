use crate::comment::skip_comment;
use crate::keywords::function_def_name;
use crate::scan::{Boundary, ScanState, boundary};

/// If `part` opens a `name() { … }` block, return the exclusive end position
/// and whether the block was closed. `in_quote` is the quote state at `start`.
pub(crate) fn scan_function_block(
    line: &[u8],
    part: &[u8],
    start: usize,
    in_quote: bool,
) -> Option<(usize, bool)> {
    function_def_name(part)?;
    let brace = line
        .get(start..)
        .and_then(|s| s.iter().position(|&b| b == b'{'))
        .map(|p| start + p)
        .unwrap_or(line.len());
    let mut quote_state = in_quote;
    let (close_pos, closed) = scan_brace_block(line, brace + 1, &mut quote_state);
    let end = line.len().min(close_pos + 1);
    Some((end, closed))
}

/// Scan forward from `i` (just after an opening `{`) to the matching `}`,
/// tracking quotes, backticks, `$( )` and nested braces. Updates `in_quote`.
/// Returns `(closing_brace_position, block_was_closed)`.
fn scan_brace_block(line: &[u8], mut i: usize, in_quote: &mut bool) -> (usize, bool) {
    let mut state = ScanState {
        in_quote: *in_quote,
        in_backtick: false,
        dollar_paren_depth: 0,
        word_active: false,
    };
    let mut depth = 1;
    while let Some(&b) = line.get(i) {
        let bare = !state.in_quote && !state.in_backtick && state.dollar_paren_depth == 0;
        if bare && b == b'{' {
            depth += 1;
            i += 1;
            continue;
        }
        if bare && b == b'}' {
            depth -= 1;
            if depth == 0 {
                break;
            }
            i += 1;
            continue;
        }
        let kind = boundary(line, i, &state);
        if kind == Boundary::Char {
            i = state.advance(line, i);
            continue;
        }
        state.word_active = false;
        if kind == Boundary::Comment {
            i = skip_comment(line, i);
        } else {
            i += 1;
        }
    }
    *in_quote = state.in_quote;
    (i, depth == 0)
}

#[cfg(test)]
mod tests;
