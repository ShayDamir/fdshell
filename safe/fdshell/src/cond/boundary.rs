//! `&&` / `||` boundary scanning for cond lists, skipping heredoc bodies so
//! a multi-line heredoc part stays one part and `&&` / `||` in its body do
//! not split it.

use crate::scan::{ScanState, heredoc};
use crate::segment::group::{closes, opens_at};

/// Scan from `i` to the next unquoted `&&` / `||` (or end of line). Heredoc
/// command lines are skipped whole along with their bodies. A `{ …; }` group
/// is opaque at this level too: the braces are counted, and an operator inside
/// the group's body is part of the body (the group's own text), so the split
/// happens after the closing brace. Returns the operator's first byte (or
/// `line.len()`) and whether it is `||`.
pub(crate) fn next_boundary(line: &[u8], mut i: usize, state: &mut ScanState) -> (usize, bool) {
    let mut run_start = i;
    let mut group_depth: i32 = 0;
    while let Some(&b) = line.get(i) {
        if !state.in_quote && state.paren_depth == 0 {
            if group_depth == 0
                && ((b == b'&' && line.get(i + 1) == Some(&b'&'))
                    || (b == b'|' && line.get(i + 1) == Some(&b'|')))
            {
                return (i, b == b'|');
            }
            if opens_at(line, i) {
                group_depth += 1;
            } else if b == b'}' && closes(b"}") {
                // A stray closer cannot drive the depth below zero.
                group_depth = group_depth.saturating_sub(1);
            }
            if b == b'\n'
                && let Some(resume) = heredoc::skip(line, run_start, i)
            {
                i = resume;
                run_start = i;
                continue;
            }
            if b == b'\n' || b == b';' {
                i += 1;
                run_start = i;
                continue;
            }
        }
        i = state.advance(line, i);
    }
    (line.len(), false)
}
