//! `&&` / `||` boundary scanning for cond lists, skipping heredoc bodies so
//! a multi-line heredoc part stays one part and `&&` / `||` in its body do
//! not split it.

use crate::scan::{ScanState, heredoc};

/// Scan from `i` to the next unquoted `&&` / `||` (or end of line). Heredoc
/// command lines are skipped whole along with their bodies. Returns the
/// operator's first byte (or `line.len()`) and whether it is `||`.
pub(crate) fn next_boundary(line: &[u8], mut i: usize, state: &mut ScanState) -> (usize, bool) {
    let mut run_start = i;
    while i < line.len() {
        let b = line.get(i).copied().unwrap_or(0);
        if !state.in_quote {
            if (b == b'&' && line.get(i + 1) == Some(&b'&'))
                || (b == b'|' && line.get(i + 1) == Some(&b'|'))
            {
                return (i, b == b'|');
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
