//! Flush the buffered runs of a physical line as statement segments,
//! attaching the line's heredoc body regions (in operator order).

use crate::scan::heredoc;
use crate::segment::Segment;
use alloc::vec::Vec;

/// Flush the buffered runs of the current physical line as statements,
/// attaching the line's heredoc body regions (in operator order). Returns the
/// resume index (end of the last body region, or the line end). A block's
/// condition bodies are handled by the block-text parse, not here.
///
/// Runs buffered from an earlier physical line (a trailing `#` comment
/// deferred the flush past the line's newline) are emitted body-less: a
/// comment-terminated operator line's body starts after *that* line, and
/// matching bodies across lines stays unsupported (the parser rejects the
/// statement, as before).
pub(crate) fn flush_line<'a>(
    segments: &mut Vec<Segment<'a>>,
    line: &[u8],
    line_start: usize,
    line_end: usize,
    runs: &mut Vec<(&'a [u8], usize)>,
) -> usize {
    let single_line = runs.first().is_none_or(|(_, off)| *off >= line_start);
    let (bodies, resume) = if single_line {
        heredoc::line_bodies_for_line(line, line_start, line_end)
    } else {
        (Vec::new(), line_end)
    };
    let mut assigned = 0;
    for (cmd, off) in runs.drain(..) {
        let n = heredoc::operator_count(line, off, off + cmd.len());
        let run_bodies: Vec<(usize, usize)> =
            bodies.iter().skip(assigned).take(n).copied().collect();
        assigned += n;
        segments.push(Segment::Statement {
            cmd,
            off,
            bodies: run_bodies,
        });
    }
    resume
}

#[cfg(test)]
mod tests;
