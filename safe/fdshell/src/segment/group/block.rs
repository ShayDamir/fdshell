//! The `{ …; }` **block builder** of the group branch: the `Segment::Block`
//! with its span, resume and floor. Extracted from `segment/group.rs` for the
//! line budget (task #456); the scan-branch bookkeeping stays in the parent.

use crate::brace::scan_brace_block;
use crate::scan::Quotes;
use crate::scan::heredoc;
use crate::segment::Segment;
use crate::segment::block::claim;
use crate::segment::group::statement_end;
use alloc::vec::Vec;

/// Build the `Segment::Block` for a `{ …; }` group whose `{` is at `start` (the
/// scanner has already landed on the brace byte, so the run's leading whitespace
/// is not part of the span). `block_start` is the span's start: the brace, or a
/// `time [-p]` prefix written before it (see `prefix::span_start`).
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
pub(super) fn group_block<'a>(
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
