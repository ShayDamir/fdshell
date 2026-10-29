//! Trailing-open check: does the buffer end in an open quote, backtick,
//! `$( )`, or a dangling top-level `&&`/`||` followed only by whitespace?

use crate::scan::last_op;

/// `true` when `line` ends in an open quote (double or single)/backtick/`$( )`,
/// or in a top-level `&&`/`||` operator followed only by whitespace.
pub(super) fn trailing_open(line: &[u8]) -> bool {
    // The shared state-aware walk (task #492), read here at the whole-buffer
    // granularity: `line.len() + 1` includes the end-of-input boundary. The
    // operator it reports is the buffer's last *token*, so an operator inside a
    // quote, a span or a comment is word data and answers "not open".
    let (state, op) = last_op::walk(line, line.len() + 1);
    if state.in_quote || state.in_single || state.in_backtick || state.paren_depth > 0 {
        return true;
    }
    // The REPL's own rule, and the reason the two granularities differ: this one
    // demands whitespace to the end of the buffer, so a `#` comment after the
    // operator is input the REPL executes at once — while the segment scanner
    // reads the same input as one cond list, because comment text is discarded
    // (task #492). Both answers are pinned.
    matches!(op, Some((pos, len)) if tail_is_whitespace(line, pos + len))
}

/// `true` when `line[from..]` is all whitespace (or empty).
fn tail_is_whitespace(line: &[u8], from: usize) -> bool {
    line.get(from..)
        .is_some_and(|tail| tail.iter().all(|b| b.is_ascii_whitespace()))
}
