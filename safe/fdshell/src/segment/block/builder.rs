//! The block builder the definer and the arithmetic-`for` head share: both are
//! head words whose `{ …; }` body the span swallows, so both land here with the
//! span end and the closed flag already decided by their own scanners.

// mutants: -C --lib -C --test -C function_keyword -C --test -C block_bodies -C --test -C block_closers -C --test -C block_conditions -- -E 'test(/^segment::/) | binary(function_keyword) | binary(block_bodies) | binary(block_closers) | binary(block_conditions)'

use super::Segment;
use super::claim;
use crate::scan::heredoc;

/// The `name() { … }` branch. Its span is the definition's text, and it claims
/// the regions of the `<<` operators written **in the definer's own text**, the
/// same rule the keyword branch applies ([`super::claim::claimed_bodies`]): a
/// multi-line body's heredocs belong to the statements inside it (**extension
/// mode** — the span keeps the block text, the body is re-scanned), while a
/// `;`-terminated operator's regions start past the closing brace (**append
/// mode**) and travel as the block's `bodies` to `run_cond_list` (task #234), so
/// the definer's own heredoc reaches the parse layer and the stored body carries
/// it (task #305).
/// The resume and the floor follow [`super::keyword::keyword_block`]'s rule so the scan
/// never resumes inside a body region: with trailing content on the line the
/// floor keeps the line's final flush from re-entering the regions claimed on
/// this line (the pre-block statements' and the body operators'), and when the
/// definition ends the line the resume is pushed past all of them. That
/// bookkeeping already treats the definer's regions as claimed, so the claim
/// never moves the cursor math.
pub(super) fn function_block(
    line: &[u8],
    start: usize,
    end: usize,
    closed: bool,
    pre_resume: usize,
    line_start: usize,
) -> (Segment<'_>, usize, usize, Option<usize>) {
    let line_end = heredoc::line_end_after(line, start);
    // `line_resume` is the end of the line's last body region past the block
    // (the line end when the bodies are missing) — the same anchor the keyword
    // branch uses.
    let (_, line_resume) = heredoc::line_bodies_for_line(line, start, line_end);
    let resume = end + 1;
    let (resume, floor) = if resume < line_end {
        (resume, line_resume.max(pre_resume))
    } else {
        (resume.max(line_resume).max(pre_resume), 0)
    };
    // The definer's own operators claim regions of the line's stream: append
    // mode for a `;`-terminated operator (its regions sit past the closing
    // brace), extension mode for the multi-line form, whose delimiter lines are
    // inside the span.
    let (bodies, end) =
        claim::claimed_bodies(line, line_start, start, end, line_end, line_resume, closed);
    (
        Segment::Block {
            block_start: start,
            end_pos: end,
            closed,
            // The definer's span ends at its own closing brace, so an unclosed
            // tail is scanned as a mid-run opener and lands on the book's flag,
            // never here (task #294; the group branch is the one that needs the
            // field, task #401).
            midrun_open: false,
            bodies,
        },
        resume,
        floor,
        // The definer's own scanner owns the brace count, so it never reads the
        // pairing's decision (task #436): nothing to report here.
        None,
    )
}
