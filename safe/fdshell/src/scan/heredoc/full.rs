//! Full heredoc body regions (body lines plus the delimiter line), so a
//! reconstructed command line plus its regions parses as a contiguous
//! heredoc at the execution layer.

use super::body::next_delimiter;
use super::op::Operator;
use alloc::vec::Vec;

/// The full body regions (body lines **plus** the delimiter line) of the
/// delimiters in order, plus the resume index (the newline terminating the
/// last delimiter line, or `line.len()` when it is last). `from` is the body
/// start, just after the command line's newline. Each region is
/// `(body_start, delimiter_line_end)`, so a reconstructed command line plus
/// its regions parses as a contiguous heredoc. `Err(n)` when the (n+1)th
/// delimiter line is missing.
pub(crate) fn body_regions_full(
    line: &[u8],
    from: usize,
    delims: &[Operator<'_>],
) -> Result<(Vec<(usize, usize)>, usize), usize> {
    let mut regions: Vec<(usize, usize)> = Vec::new();
    let mut pos = from;
    let mut resume = from;
    for (n, op) in delims.iter().enumerate() {
        let Some((_start, end)) = next_delimiter(line, pos, op) else {
            return Err(n);
        };
        // Include the newline terminating the delimiter line so the
        // reconstructed command line parses as a contiguous heredoc.
        let region_end = (end + 1).min(line.len());
        regions.push((pos, region_end));
        resume = end;
        pos = end + 1;
    }
    Ok((regions, resume))
}

#[cfg(test)]
mod tests;
