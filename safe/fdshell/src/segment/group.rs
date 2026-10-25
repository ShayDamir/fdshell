use crate::brace::scan_brace_block;
use crate::scan::Quotes;
use crate::scan::heredoc;
use crate::segment::block::claim;
use crate::segment::book::Book;
use crate::segment::line;
use alloc::vec::Vec;

use super::Segment;

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

/// Build the `Segment::Block` for a `{ …; }` group whose `{` is at `start` (the
/// scanner has already landed on the brace byte, so the run's leading whitespace
/// is not part of the span). `block_start` is the span's start: the brace, or a
/// `time [-p]` prefix written before it (see [`prefix::span_start`]).
/// The span runs from the `{` to the end of the
/// statement it opens, so the group owns everything written after its closing
/// brace (`} > out`, `} && echo b`, `} | cat`): the parse layer decides what the
/// trailing operators mean.
/// Returns the segment, the scan resume index, and the resume floor for the
/// rest of the physical line (past which no flush may resume).
/// `pre_resume` is the end of the body regions of a statement that precedes
/// the group on the same line (the line end when there is none); `line_start`
/// is the start of the physical line whose body stream the group reads.
///
/// The group claims **its own** regions of that stream and the resume/floor
/// follow the keyword branch's rule (`segment/block/keyword.rs::keyword_block`),
/// exactly like the definer branch (task #305): a `;`-terminated operator in
/// the group's **body** text
/// (`{ cat <<T; }`) claims regions written past the closing brace, and they
/// travel as the segment's `bodies` to `run_cond_list` (task #234), so the
/// group body's `<<` reaches the parse layer.
/// The **extension** arm of [`claim::claimed_bodies`] is reachable here only by
/// a multi-line group whose first physical line carries the operator
/// (`{ cat <<T;\ncat <<U;\n}`): its regions are interleaved with the group's
/// own lines, the span keeps the whole group text and carries no bodies. A
/// **closed single-line** group's regions always start at/after its closing
/// separator, and a multi-line group whose opener line has no operator claims
/// nothing at all, so the arm never moves `end_pos` for those two shapes — the
/// mode is asserted by the pins in `group/tests.rs`, not padded (task #311's
/// rule).
fn group_block<'a>(
    line: &'a [u8],
    start: usize,
    block_start: usize,
    quotes: Quotes,
    pre_resume: usize,
    line_start: usize,
) -> (Segment<'a>, usize, usize) {
    let mut quotes = quotes;
    let (close, closed) = scan_brace_block(line, start + 1, &mut quotes);
    // The group never closes: the span is the rest of the input, and the REPL
    // keeps reading continuation lines.
    let open_end = line.len().min(close + 1);
    let (end, resume, floor, bodies) = if closed {
        let (sep, r) = statement_end(line, close + 1, quotes);
        let line_end = heredoc::line_end_after(line, start);
        // `line_resume` is the end of the line's last body region (the line end
        // when the bodies are missing) — the same anchor the keyword branch uses.
        let (_, line_resume) = heredoc::line_bodies_for_line(line, line_start, line_end);
        let (bodies, end) = claim::claimed_bodies(
            line,
            line_start,
            block_start,
            sep,
            line_end,
            line_resume,
            true,
        );
        let (resume, floor) = if r < line_end {
            // Trailing content on the group's line: scan it; the line's final
            // flush must not re-enter any body region claimed on this line.
            (r.max(pre_resume), line_resume.max(pre_resume))
        } else {
            // The group ends its physical line: resume past every region.
            (r.max(line_resume).max(pre_resume), 0)
        };
        (end, resume, floor, bodies)
    } else {
        (open_end, open_end + 1, 0, Vec::new())
    };
    (
        Segment::Block {
            block_start,
            end_pos: end,
            closed,
            bodies,
        },
        resume,
        floor,
    )
}

#[cfg(test)]
mod tests;
