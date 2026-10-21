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
    /// delimiter_line_end)`; the first missing one's **rest of the stream**
    /// `(pos, line.len())`; every operator behind it the empty region at the
    /// stream end. `pos` is one past `line.len()` when the last delimiter line
    /// has no terminating newline, so a tail region's range can be reversed
    /// (`start > end`) — the bytes it stands for are then empty. **Empty when
    /// the first operator's delimiter line is missing**, or when the line has
    /// no readable operator: the line claims nothing and the parse layer
    /// reports the shape.
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
            // The line's *first* operator has no delimiter line: nothing is
            // claimed, so the parse layer owns "this heredoc has no body".
            None if alloc.found == 0 => return Allocation::empty(from),
            // The first missing operator claims the rest of the stream; the
            // stream being spent, operators behind it claim its empty tail.
            None => {
                alloc.regions.push((pos, line.len()));
                alloc.resume = line.len();
                pos = line.len();
            }
        }
    }
    alloc
}

impl Allocation {
    fn empty(from: usize) -> Self {
        Self {
            regions: Vec::new(),
            resume: from,
            found: 0,
        }
    }
}

/// The bytes each region stands for: the region's own text, and for every
/// region past `found` — the operators whose delimiter line never arrived —
/// that delimiter line **synthesized** (`op.delim` on a line of its own). The
/// rebuild (`cmd + "\n" + body`) then parses as one complete heredoc whose body
/// is what the stream left, so the parse layer's own allocation *finds* the
/// synthesized line and never appends it a second time (LESSONS: the synthesis
/// point is the region→bytes boundary).
pub(crate) fn contents(line: &[u8], delims: &[Operator], alloc: &Allocation) -> Vec<Vec<u8>> {
    let last = alloc.regions.len().saturating_sub(1);
    alloc
        .regions
        .iter()
        .enumerate()
        .map(|(n, &(s, e))| region_bytes(line.get(s..e), n, last, delims, alloc.found))
        .collect()
}

/// One region's bytes: its own text, plus the synthesized delimiter line when
/// its operator sits past `found`.
///
/// The whole-line closure is load-bearing: a text that ends mid-line (a `-c`
/// script with no trailing newline leaves its last delimiter line unterminated)
/// hands the region bytes with no final newline, and the bodies are
/// **concatenated** — the missing newline would otherwise glue the next body's
/// first byte onto this region's delimiter line. So every region that is
/// followed by another one, and every synthesized region (whose delimiter line
/// needs a line start of its own), is closed; the last region stays byte-exact.
fn region_bytes(
    span: Option<&[u8]>,
    n: usize,
    last: usize,
    delims: &[Operator],
    found: usize,
) -> Vec<u8> {
    let mut bytes = span.unwrap_or_default().to_vec();
    let synthesized = n >= found;
    if !bytes.is_empty() && !bytes.ends_with(b"\n") && (synthesized || n < last) {
        bytes.push(b'\n');
    }
    if synthesized {
        let delim = delims
            .get(n)
            .map(|o| o.delim.as_slice())
            .unwrap_or_default();
        bytes.extend_from_slice(delim);
        bytes.push(b'\n');
    }
    bytes
}

#[cfg(test)]
mod tests;
