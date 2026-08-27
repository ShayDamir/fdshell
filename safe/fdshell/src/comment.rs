use alloc::vec::Vec;

use crate::scan::{Boundary, ScanState, boundary, skip_comment};

/// Sum the block-depth deltas of the keywords in a space-separated word run.
///
/// A keyword only counts when its word is unquoted: a `for` / `if` / … inside
/// double quotes, backticks, or a `$( )` substitution is data, not a nested
/// block opener. The quote / backtick / `$( )` handling mirrors
/// `ScanState::advance` so the per-word state matches the surrounding scan.
/// A run always starts at substitution depth 0 (separators only exist at
/// depth 0), so only the caller's quote state is taken as a parameter.
fn depth_delta(raw: &[u8], in_quote: bool, in_backtick: bool) -> i32 {
    let mut sum = 0;
    let mut frag = Vec::new();
    let mut in_quote = in_quote;
    let mut in_backtick = in_backtick;
    let mut depth: u32 = 0;
    // A run starts quoted only when the caller is mid-quote/mid-backtick;
    // the flag is sticky until a split (splits only happen unquoted, so a
    // quoted word is one fragment and can never be a bare keyword).
    let mut frag_quoted = in_quote || in_backtick;
    let mut i = 0usize;
    while let Some(&b) = raw.get(i) {
        let bare = !in_quote && !in_backtick;
        let mut step = 1usize;
        if b == b'"' {
            in_quote = !in_quote;
        } else if bare && b == b'$' && raw.get(i + 1) == Some(&b'(') {
            depth = depth.saturating_add(1);
            step = 2;
        } else if bare && b == b'(' && depth > 0 {
            depth = depth.saturating_add(1);
        } else if bare && b == b')' {
            depth = depth.saturating_sub(1);
        } else if b == b'`' && bare != in_backtick {
            in_backtick = bare;
        } else if b == b' ' && bare && depth == 0 {
            // A split only exists outside quotes and substitutions, so the
            // next word always starts unquoted.
            if !frag_quoted {
                sum += crate::keywords::keyword_delta(&frag).unwrap_or(0);
            }
            frag.clear();
            frag_quoted = false;
            i += step;
            continue;
        }
        frag.push(b);
        i += step;
    }
    if !frag_quoted {
        sum += crate::keywords::keyword_delta(&frag).unwrap_or(0);
    }
    sum
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
        depth = depth.saturating_add_signed(depth_delta(raw, run_quote, run_backtick));
        if kind == Boundary::Comment {
            i = skip_comment(line, i);
            *start = i;
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
