//! End-of-physical-line handling: the newline and `#` comment endings, whose
//! run flush lives in [`flush`].

// mutants: -C --lib -- -E 'test(/^segment::/)'

use crate::scan;
use crate::scan::heredoc;
use crate::segment::bg;
use crate::segment::book::Book;

pub(super) mod flush;

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
    let resume = flush::flush_line(book, line, line_start, line_end);
    book.line_start = resume;
    resume
}

/// A `#` comment **ends the physical line**, so the line keeps the end-of-line
/// bookkeeping a newline gets: its buffered runs are flushed with **that
/// line's** body regions — bash reads a here-document body after the *logical*
/// line, comment included, so a `#`-terminated line owns a body stream — and
/// the cursor lands past every region the line claimed, never inside one. The
/// deferral a next-line-opening `&` needs is the newline arm's rule too, gate
/// included: a line that claims a body region ends it with the region attached,
/// and the `&` line below is region data.
pub(super) fn end_comment(book: &mut Book<'_>, line: &[u8]) {
    // The flush window ends at the comment's own newline, so the comment's
    // bytes are never read as operators.
    let nl = scan::comment_end(line, book.i);
    // `async_ahead` only answers true when `nl` sits on a real newline, so the
    // equality conjunct it used to carry is implied and deleted (rule 3).
    let deferred = bg::async_ahead(line, nl) && heredoc::skip(line, book.line_start, nl).is_none();
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

#[cfg(test)]
mod tests;
