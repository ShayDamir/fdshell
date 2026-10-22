//! Flush the buffered runs of a physical line as statement segments,
//! attaching the line's heredoc body bytes (in operator order).

use crate::scan::heredoc;
use crate::segment::Segment;
use crate::segment::book::Book;
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
    let (line_start, line_end) = (book.line_start, book.i);
    let resume = flush_line(book, line, line_start, line_end);
    book.line_start = resume;
    resume
}

/// Flush the book's buffered runs of the current physical line as statements,
/// attaching the line's heredoc body bytes (in operator order). Returns the
/// resume index (the end of the last claimed region — the text end when a
/// missing delimiter line claimed the rest of the stream — or the line end). A
/// block's condition bodies are handled by the block-text parse, not here.
/// The flush empties the book's runs, and stamps them with its mid-run flag:
/// an unclosed mid-run construct resumes at the end of input, so the whole
/// physical line stays unfinished and every run of the flush is stamped
/// (task #294).
///
/// Runs buffered from an earlier physical line (a trailing `#` comment
/// deferred the flush past the line's newline) are emitted body-less: a
/// comment-terminated operator line's body starts after *that* line, and
/// matching bodies across lines stays unsupported (the parser rejects the
/// statement, as before).
pub(crate) fn flush_line<'a>(
    book: &mut Book<'a>,
    line: &[u8],
    line_start: usize,
    line_end: usize,
) -> usize {
    // No runs to emit: the line's body regions (if any) belong to the block
    // the caller is about to scan, not to a statement — claim nothing.
    let mut runs = core::mem::take(&mut book.runs);
    if runs.is_empty() {
        return line_end;
    }
    let midrun_open = book.midrun_open;
    let single_line = runs.first().is_none_or(|run| run.off >= line_start);
    let (bodies, resume) = if single_line {
        heredoc::line_body_contents(line, line_start, line_end)
    } else {
        (Vec::new(), line_end)
    };
    // Body bytes already claimed by operators before the first run: a block's
    // condition operators, when the runs trail a block on the same line (the
    // line's body stream is global, in operator order).
    let mut assigned = runs
        .first()
        .map(|run| heredoc::operator_count(line, line_start, run.off))
        .unwrap_or(0);
    for run in runs.drain(..) {
        let n = heredoc::operator_count(line, run.off, run.off + run.bytes.len());
        let run_bodies: Vec<Vec<u8>> = bodies.iter().skip(assigned).take(n).cloned().collect();
        assigned += n;
        book.segments.push(Segment::Statement {
            cmd: run.bytes,
            off: run.off,
            bodies: run_bodies,
            background: run.background,
            midrun_open,
        });
    }
    resume
}

#[cfg(test)]
mod tests;
