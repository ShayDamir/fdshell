//! Flush the buffered runs of a physical line as statement segments,
//! attaching the line's heredoc body regions (in operator order).

use crate::scan::heredoc;
use crate::segment::Segment;
use crate::segment::book::{Book, Run};
use alloc::vec::Vec;

/// End of a physical line: flush its buffered runs, or defer the flush when
/// the next line opens with the plain `&` (the runs stay buffered so that
/// `&` can claim them). A flush ends the `;` chapter: the flushed statement
/// is a `&` target again. Returns the resume index.
pub(super) fn end_line(book: &mut Book<'_>, line: &[u8], deferred: bool) -> usize {
    book.sep_since_run = deferred;
    if deferred {
        return book.i;
    }
    let resume = flush_line(
        &mut book.segments,
        line,
        book.line_start,
        book.i,
        &mut book.runs,
    );
    book.line_start = resume;
    resume
}

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
    runs: &mut Vec<Run<'a>>,
) -> usize {
    // No runs to emit: the line's body regions (if any) belong to the block
    // the caller is about to scan, not to a statement — claim nothing.
    if runs.is_empty() {
        return line_end;
    }
    let single_line = runs.first().is_none_or(|run| run.off >= line_start);
    let (bodies, resume) = if single_line {
        heredoc::line_bodies_for_line(line, line_start, line_end)
    } else {
        (Vec::new(), line_end)
    };
    // Regions already claimed by operators before the first run: a block's
    // condition operators, when the runs trail a block on the same line (the
    // line's body stream is global, in operator order).
    let mut assigned = runs
        .first()
        .map(|run| heredoc::operator_count(line, line_start, run.off))
        .unwrap_or(0);
    for run in runs.drain(..) {
        let n = heredoc::operator_count(line, run.off, run.off + run.bytes.len());
        let run_bodies: Vec<(usize, usize)> =
            bodies.iter().skip(assigned).take(n).copied().collect();
        assigned += n;
        segments.push(Segment::Statement {
            cmd: run.bytes,
            off: run.off,
            bodies: run_bodies,
            background: run.background,
        });
    }
    resume
}

#[cfg(test)]
mod tests;
