use crate::scan::heredoc;
use crate::segment::book::Book;
use crate::segment::line;
use block::group_block;

mod block;
pub(super) mod opener;
mod prefix;
mod span;
mod stage;
mod word;
pub(crate) use prefix::{definer_at_run, group_at_run};
pub(super) use span::{skip_group, statement_end};
pub(crate) use stage::{command_at_run, stage_at_run};
pub(crate) use word::{closes, opens, opens_at};

/// Take the group branch of the segment scan at the group opener of the run
/// that starts at `book.start`: the group's span is pushed. The opener is the
/// run's first `{` — the scan only gets here when the run before it is empty
/// or a cond operator, so nothing before that brace can be one. The cursor is
/// rewound past the group's span with the line's resume floor.
pub(super) fn open<'a>(line: &'a [u8], book: &mut Book<'a>) {
    let (run_start, line_start) = (book.start, book.line_start);
    let tail = line.get(run_start..).unwrap_or(b"");
    let brace = run_start + tail.iter().position(|&b| b == b'{').unwrap_or(0);
    // The opener is at a run start, so nothing precedes it on this run; the
    // flush claims only the line's heredoc bodies scanned so far. The book's
    // mid-run flag is `false` here: an unclosed mid-run span resumes at the end
    // of input, so no group opener follows it.
    let pre_resume = line::flush_line(book, line, line_start, brace);
    let span_start = prefix::span_start(line, run_start, brace);
    let (segment, resume, floor) = group_block(
        line,
        brace,
        span_start,
        book.state.quotes(),
        pre_resume,
        line_start,
    );
    book.segments.push(segment);
    // The group's span ends the **statement** it opens, so it ends the physical
    // line only when the statement does: a group whose statement stops at a `;`
    // mid-line has trailing commands after it, and its own resume is right
    // there. When the span does end the line, the floor the line already carries
    // (a heredoc region claimed by a block before the group) outranks the
    // resume, so the scan never lands inside that region ([`Book::rewind`],
    // task #305).
    let line_end = heredoc::line_end_after(line, brace);
    // Trailing content shares the group's physical line, so its runs' bookkeeping
    // — and the global body stream — continues from the **line's** start: the
    // trailing flush's `assigned` count then skips the regions the group's own
    // operators claimed (`{ cat <<T; }; cat <<U` gives the `cat <<U` run region
    // 1, not the group's region 0). The keyword/definer branches do the same
    // ([`super::block::open_block`], tasks #240/#305).
    let new_start = if resume < line_end {
        line_start
    } else {
        resume
    };
    book.rewind(resume, new_start, floor, resume >= line_end);
}

#[cfg(test)]
mod tests;
