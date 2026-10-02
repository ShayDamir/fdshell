//! Trailing-open check: does the buffer end in an open quote, backtick,
//! `$( )`, or a dangling top-level `&&`/`||` followed only by whitespace?

use crate::scan::{Boundary, ScanState, boundary, heredoc, skip_comment};

/// `true` when `line` ends in an open quote/backtick/`$( )`, or in a
/// top-level `&&`/`||` operator followed only by whitespace.
pub(super) fn trailing_open(line: &[u8]) -> bool {
    let mut state = ScanState::new();
    let mut i = 0usize;
    let mut run_start = 0usize;
    let mut last_op: Option<(usize, usize)> = None;
    while i <= line.len() {
        let kind = boundary(line, i, &state);
        if kind == Boundary::Char {
            if is_bare(&state)
                && let Some(len) = top_level_op(line, i)
            {
                last_op = Some((i, len));
                for _ in 0..len {
                    i = state.advance(line, i);
                }
                continue;
            }
            i = state.advance(line, i);
            continue;
        }
        state.word_active = false;
        if kind == Boundary::Comment {
            i = skip_comment(line, i);
            run_start = i;
            continue;
        }
        match heredoc::skip_region(line, run_start, i) {
            Some((_, resume, _)) => {
                i = resume;
                run_start = i;
            }
            None => {
                i += 1;
                run_start = i;
            }
        }
    }
    if state.in_quote || state.in_backtick || state.paren_depth > 0 {
        return true;
    }
    matches!(last_op, Some((pos, len)) if tail_is_whitespace(line, pos + len))
}

/// `true` when the state is at the top level (no open quote, backtick, or
/// `$( )`/`(( ))` substitution).
fn is_bare(state: &ScanState) -> bool {
    !state.in_quote && !state.in_backtick && state.paren_depth == 0
}

/// The length of the top-level cond-list operator at `i`: `&&` (2) or `||`
/// (2). A lone `&` is not an operator (fdshell has no backgrounding), and a
/// lone `|` is not a continuation (the pipeline parser cannot span a `|`
/// across a newline, so a trailing `|` stays a hard parse error).
fn top_level_op(line: &[u8], i: usize) -> Option<usize> {
    match line.get(i).copied() {
        Some(b'&') if line.get(i + 1) == Some(&b'&') => Some(2),
        Some(b'|') if line.get(i + 1) == Some(&b'|') => Some(2),
        _ => None,
    }
}

/// `true` when `line[from..]` is all whitespace (or empty).
fn tail_is_whitespace(line: &[u8], from: usize) -> bool {
    line.get(from..)
        .is_some_and(|tail| tail.iter().all(|b| b.is_ascii_whitespace()))
}
