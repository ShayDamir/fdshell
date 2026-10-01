//! Completeness check for a REPL input buffer: is it a complete construct,
//! or does it need more lines (continuation)?
//!
//! The check reuses the existing byte scanners verbatim (no parser changes):
//! `scan_segments` reports open keyword/function blocks and heredoc command
//! lines, and a `ScanState` walk reports open quotes/backticks/`$( )` and a
//! dangling top-level `&&`/`||`.

use crate::scan::{Boundary, ScanState, boundary, heredoc, skip_comment};
use crate::segment::{Segment, scan_segments};

/// `true` when `line` is a complete construct and can be executed now;
/// `false` when it is incomplete (an open block, an unterminated heredoc, a
/// trailing operator, or an unbalanced quote/backtick/`$( )`) and the REPL
/// should read a continuation line.
pub(crate) fn is_complete(line: &[u8]) -> bool {
    !blocks_or_heredoc_open(line) && !trailing_open(line)
}

/// `true` when `line` carries an open keyword/function block or an
/// unterminated heredoc.
fn blocks_or_heredoc_open(line: &[u8]) -> bool {
    for seg in scan_segments(line, false) {
        match seg {
            Segment::Block { closed: false, .. } => return true,
            Segment::Statement(_, off)
                if heredoc::unterminated(line, off, cmd_line_end(line, off)) =>
            {
                return true;
            }
            // A closed block (or any other segment) is complete.
            _ => {}
        }
    }
    false
}

/// The end of the command line starting at `off`: the first separator (`;`
/// or `\n`) at or after `off` (or end of input). A command line never
/// contains a separator, so this is the position of the byte that ends it.
/// Using the command line's own terminator (not the end of a multi-line
/// heredoc span) is what lets `unterminated` find a delimiter line that
/// follows the command line; a `;` terminator makes the run a hard parse
/// error, not a continuation.
fn cmd_line_end(line: &[u8], off: usize) -> usize {
    line.get(off..)
        .and_then(|s| s.iter().position(|&b| b == b'\n' || b == b';'))
        .map(|p| off + p)
        .unwrap_or(line.len())
}

/// `true` when `line` ends in an open quote/backtick/`$( )`, or in a
/// top-level `&&`/`||` operator followed only by whitespace.
fn trailing_open(line: &[u8]) -> bool {
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

#[cfg(test)]
mod tests;
