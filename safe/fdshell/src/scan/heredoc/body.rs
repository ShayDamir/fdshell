//! Heredoc body-span resolution: where each delimiter line starts and where
//! the body ends.

use super::op::Operator;
use alloc::vec::Vec;

/// The `(body_start, delimiter_line_start)` spans of the delimiters in order
/// plus the resume index (the delimiter line's terminating newline, or
/// `line.len()` when the delimiter line is last). `from` is the body start,
/// just after the command line's newline. `Err(n)` when the (n+1)th
/// delimiter line is missing.
pub(crate) fn body_spans(
    line: &[u8],
    from: usize,
    delims: &[Operator],
) -> Result<(Vec<(usize, usize)>, usize), usize> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut pos = from;
    let mut resume = from;
    for (n, op) in delims.iter().enumerate() {
        let Some((start, end)) = next_delimiter(line, pos, op) else {
            return Err(n);
        };
        spans.push((pos, start));
        resume = end;
        pos = end + 1;
    }
    Ok((spans, resume))
}

/// The next line matching `op` starting at `from`: the delimiter line's
/// start and the index of the newline terminating it (or `line.len()` when
/// the line is last). A zero-byte final line never matches, so an empty
/// delimiter needs a real blank line (a tabs-only line strips to empty and
/// does match a `<<-""` body).
pub(super) fn next_delimiter(line: &[u8], from: usize, op: &Operator) -> Option<(usize, usize)> {
    let mut i = from;
    while i < line.len() {
        let nl = line
            .get(i..)
            .and_then(|s| s.iter().position(|&b| b == b'\n'))
            .map(|p| i + p);
        let j = nl.unwrap_or(line.len());
        if op.matches(line.get(i..j)?) {
            return Some((i, j));
        }
        let n = nl?;
        i = n + 1;
    }
    None
}
