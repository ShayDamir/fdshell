//! Full heredoc body regions (body lines plus the delimiter line), so a
//! reconstructed command line plus its regions parses as a contiguous
//! heredoc at the execution layer. Allocation is **greedy, one entry per
//! operator** (bash's rule): a missing delimiter line starves its own operator
//! and never the operators whose body is complete.

use super::body::next_delimiter;
use super::op::Operator;
use alloc::vec::Vec;

/// One physical line's body allocation, laid out in operator order.
pub(crate) struct Allocation {
    /// One region per operator: a found operator's `(body_start,
    /// delimiter_line_end)`; a missing one's **rest of the stream**
    /// `(pos, line.len())`; every operator behind it the empty region at the
    /// stream end. `pos` is one past `line.len()` when the last delimiter line
    /// has no terminating newline, so a tail region's range can be reversed
    /// (`start > end`) — the bytes it stands for are then empty. Empty only
    /// when the line has no readable operator: the line claims nothing and the
    /// parse layer owns the shape (task #364).
    pub(crate) regions: Vec<(usize, usize)>,
    /// End of the last claimed region: `line.len()` when a missing-delimiter
    /// region exists (its bytes are body data, never commands), `from` when
    /// nothing is claimed, else the last delimiter line's newline.
    pub(crate) resume: usize,
    /// How many operators found their delimiter line.
    pub(crate) found: usize,
}

/// Allocate `line`'s body regions for `delims`, in operator order, the stream
/// starting at `from` (just after the command line's newline): each operator
/// starts where the previous operator's delimiter line ended, and the first
/// missing one takes what is left of the stream.
pub(crate) fn body_regions_full(line: &[u8], from: usize, delims: &[Operator]) -> Allocation {
    let mut alloc = Allocation {
        regions: Vec::new(),
        resume: from,
        found: 0,
    };
    let mut pos = from;
    for op in delims {
        match next_delimiter(line, pos, op) {
            // Include the newline terminating the delimiter line so the
            // reconstructed command line parses as a contiguous heredoc.
            Some((_start, end)) => {
                alloc.regions.push((pos, (end + 1).min(line.len())));
                alloc.resume = end;
                alloc.found += 1;
                pos = end + 1;
            }
            // The first missing operator claims the rest of the stream — its
            // **end-of-file** body (task #364) — and the stream being spent,
            // operators behind it claim its empty tail. The claim carries no
            // synthesized delimiter line: the parse layer reads the rebuild's own
            // text, decides the shape itself, and prints the warning.
            None => {
                alloc.regions.push((pos, line.len()));
                alloc.resume = line.len();
                pos = line.len();
            }
        }
    }
    alloc
}

/// The bytes each region stands for: the region's own text, and nothing else. A
/// region whose delimiter line never arrived is the end-of-file shape and gets no
/// synthesized delimiter line (task #364), so the rebuild (`cmd + "\n" + body`) is
/// a text the parse layer can still decide for — and print bash's warning from.
/// Synthesizing the line here would hand it a heredoc that looks complete and move
/// the diagnostic to this byte layer, which only execution-path callers reach.
pub(crate) fn contents(line: &[u8], alloc: &Allocation) -> Vec<Vec<u8>> {
    let last = alloc.regions.len().saturating_sub(1);
    alloc
        .regions
        .iter()
        .enumerate()
        .map(|(n, &(s, e))| region_bytes(line.get(s..e), n, last))
        .collect()
}

/// One region's bytes: its own text, closed to a whole line when another region
/// follows it.
///
/// The whole-line closure is load-bearing: a text that ends mid-line (a `-c`
/// script with no trailing newline leaves its last delimiter line unterminated)
/// hands the region bytes with no final newline, and the bodies are
/// **concatenated** — the missing newline would otherwise glue the next body's
/// first byte onto this region's last line. So every region that is followed by
/// another one is closed; the last region stays byte-exact.
fn region_bytes(span: Option<&[u8]>, n: usize, last: usize) -> Vec<u8> {
    let mut bytes = span.unwrap_or_default().to_vec();
    if !bytes.is_empty() && !bytes.ends_with(b"\n") && n < last {
        bytes.push(b'\n');
    }
    bytes
}

#[cfg(test)]
mod tests;
