//! Flush the buffered runs of a physical line as statement segments,
//! attaching the line's heredoc body bytes (in operator order).

use crate::scan::{self, heredoc};
use crate::segment::Segment;
use crate::segment::bg;
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

/// A `#` comment **ends the physical line**, so the line keeps the end-of-line
/// bookkeeping a newline gets: its buffered runs are flushed with **that
/// line's** body regions — bash reads a here-document body after the *logical*
/// line, comment included, so a `#`-terminated line owns a body stream — and
/// the cursor lands past every region the line claimed, never inside one. The
/// deferral a next-line-opening `&` needs is the newline arm's rule too.
pub(super) fn end_comment(book: &mut Book<'_>, line: &[u8]) {
    // The flush window ends at the comment's own newline, so the comment's
    // bytes are never read as operators.
    let nl = scan::comment_end(line, book.i);
    let deferred = line.get(nl) == Some(&b'\n') && bg::async_ahead(line, nl);
    book.i = nl;
    // `skip_comment` is the newline's successor: the next physical line's
    // start, the address of a line that claims no region at all.
    let next_line = scan::skip_comment(line, nl);
    let resume = end_line(book, line, deferred)
        .max(next_line)
        .max(book.body_floor);
    book.i = resume;
    book.start = resume;
    // The regions this line claimed are gone for every later flush: its runs are
    // emitted, and its body lines are data.
    book.line_start = resume;
    book.body_floor = resume;
    // A comment closes the `;` chapter: the flushed runs stay plain-`&` targets.
    book.sep_since_run = false;
}

/// Flush the book's buffered runs of the current physical line as statements,
/// attaching the line's heredoc body bytes (in operator order). Returns the
/// resume index (the end of the last claimed region — the text end when a
/// missing delimiter line claimed the rest of the stream — or the line end). A
/// block's condition bodies are handled by the block-text parse, not here.
/// The flush empties the book's runs, and stamps them with its mid-run flag and
/// its join flag: an unclosed mid-run construct resumes at the end of input, so
/// the whole physical line stays unfinished and every run of the flush is stamped
/// (task #294). The join flag goes on the flush's **last** run — the run whose own
/// text ended on the operator's terminator, the one the decision was made for —
/// and the `;`-separated runs ahead of it keep their own `(text, bodies)` pair
/// (task #492).
///
/// Runs buffered from an **earlier physical line** — the flush was deferred past
/// the line's newline by a line-opening `&` (`bg::async_ahead`), or the run
/// stayed open across a newline because its last token is a top-level `&&`/`||`
/// (POSIX #2.9's AND-IF-TERMINATOR, task #492) — are emitted body-less: a run of
/// an earlier line must never index *this* line's region list, and the run's own
/// line is unidentifiable once `line_start` has moved, so the `single_line` arm
/// below is the total-function defense. A trailing `#` comment no longer reaches
/// here: [`end_comment`] ends the physical line and flushes its runs **on** it,
/// because a `#`-terminated line owns its body stream (task #333 — bash reads the
/// body after the logical line, comment included). The rule is *not* about a run
/// whose own text spans several physical lines: a keyword block opened mid-run
/// keeps its bytes inside the run (task #232) and never moves `line_start`, so
/// the run is flushed on the line it opened on and claims that line's region
/// stream like any single-line run (task #295). A run joined across a newline
/// (task #492) is the exception: its window spans every physical line the run
/// runs over, and the run joins only when that window claims **no** region at all
/// (extension mode, nothing claimed). A line that claims one keeps its own segment
/// with its own bodies, the run the decision was made for carries the mark, and the
/// runner concatenates the chain (`script/merge.rs`).
pub(crate) fn flush_line<'a>(
    book: &mut Book<'a>,
    line: &[u8],
    line_start: usize,
    line_end: usize,
) -> usize {
    // No runs to emit: the line's body regions (if any) belong to the block
    // the caller is about to scan, not to a statement — claim nothing.
    let mut runs = core::mem::take(&mut book.runs);
    // The join flag belongs to the run that ended on the operator's terminator,
    // and this is the flush that stamps it (task #492).
    let join_next = core::mem::replace(&mut book.join_next, false);
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
    // The flag belongs to the run the decision was made for — the one whose own
    // text ends on the terminator, which is the flush's last run. A `;`-separated
    // run ahead of it keeps its own `(text, bodies)` pair and its own statement
    // (task #492): the join never reaches back over a `;`.
    let last = runs.len();
    for (index, run) in runs.drain(..).enumerate() {
        let n = heredoc::operator_count(line, run.off, run.off + run.bytes.len());
        let run_bodies: Vec<Vec<u8>> = bodies.iter().skip(assigned).take(n).cloned().collect();
        assigned += n;
        book.segments.push(Segment::Statement {
            cmd: run.bytes,
            off: run.off,
            bodies: run_bodies,
            background: run.background,
            midrun_open,
            join_next: join_next && index + 1 == last,
        });
    }
    resume
}

#[cfg(test)]
mod tests;
