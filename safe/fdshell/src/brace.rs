use crate::keywords::function_def_name;
use crate::scan::{Boundary, Quotes, boundary, heredoc, skip_comment};

/// If `part` opens a `name() { … }` block, return the exclusive end position
/// and whether the block was closed. `quotes` is the quote state at `start`.
pub(crate) fn scan_function_block(
    line: &[u8],
    part: &[u8],
    start: usize,
    quotes: Quotes,
) -> Option<(usize, bool)> {
    function_def_name(part, line, start)?;
    let brace = line
        .get(start..)
        .and_then(|s| s.iter().position(|&b| b == b'{'))
        .map(|p| start + p)
        .unwrap_or(line.len());
    let mut quotes = quotes;
    let (close_pos, closed) = scan_brace_block(line, brace + 1, &mut quotes);
    let end = line.len().min(close_pos + 1);
    Some((end, closed))
}

/// Scan forward from `i` (just after an opening `{`) to the matching `}`,
/// tracking quotes, backticks, `$( )` and nested braces. Updates `quotes`.
/// Returns `(closing_brace_position, block_was_closed)`.
///
/// Two callers share it: the `name() { … }` function body above, and the
/// `{ …; }` group command in `segment/group.rs`.
pub(crate) fn scan_brace_block(line: &[u8], mut i: usize, quotes: &mut Quotes) -> (usize, bool) {
    let mut state = quotes.scan_state();
    let mut depth = 1;
    let mut run_start = i;
    while let Some(&b) = line.get(i) {
        let bare =
            !state.in_quote && !state.in_single && !state.in_backtick && state.paren_depth == 0;
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
        } else if let Some(resume) = heredoc::skip(line, run_start, i) {
            // A heredoc body is opaque: braces in it do not close the block.
            i = resume;
        } else {
            i += 1;
        }
        run_start = i;
    }
    *quotes = state.quotes();
    (i, depth == 0)
}

#[cfg(test)]
mod tests;
