//! Flush the buffered runs of a physical line as statement segments,
//! attaching the line's heredoc body bytes (in operator order).

use crate::scan::heredoc;
use crate::segment::Segment;
use crate::segment::book::{Book, Run};
use alloc::vec::Vec;

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
/// here: [`super::end_comment`] ends the physical line and flushes its runs **on** it,
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
    // The separator join points recorded so far belong to the runs of **this**
    // flush — a run claims only the ones inside its own bytes (`run_joins`).
    let joins = core::mem::take(&mut book.sep_joins);
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
        let end = run.off + run.bytes.len();
        let n = heredoc::operator_count(line, run.off, end);
        let run_bodies: Vec<Vec<u8>> = if spans_lines(line, run.off, end) {
            heredoc::run_line_bodies(line, end)
        } else {
            bodies.iter().skip(assigned).take(n).cloned().collect()
        };
        assigned += n;
        book.segments.push(Segment::Statement {
            cmd: run.bytes,
            off: run.off,
            bodies: run_bodies,
            background: run.background,
            midrun_open,
            join_next: join_next && index + 1 == last,
            joins: run_joins(&joins, &run),
        });
    }
    resume
}

/// The separator join points of one run (task #492), as offsets into the run's own
/// bytes: a run claims only the newlines that fall inside its span, so a
/// `;`-separated run never takes a join point belonging to the run beside it. The
/// runner writes the claimed ones as spaces when it folds a chain
/// (`script/merge/chain.rs`).
fn run_joins(joins: &[usize], run: &Run<'_>) -> Vec<usize> {
    let end = run.off + run.bytes.len();
    joins
        .iter()
        .filter(|at| **at >= run.off && **at < end)
        .map(|at| at - run.off)
        .collect()
}

/// Whether the run's bytes `line[run_start..run_end]` span more than one physical
/// line — a run whose bytes cross a newline is a mid-run keyword block (task #232),
/// and its `<<` operators are written on its **own internal** lines. Each physical
/// line inside such a run owns exactly one body stream and hands its regions out in
/// operator order, so the allocations come out in global per-line order — the single
/// definition of an opaque body, the same rule the whole-text scan
/// (`scan/heredoc/regions.rs`) and `segment/block/claim.rs` apply, pinned by
/// `tests/cond_blocks.rs::multi_line_mid_run_runs_consume_their_own_lines_streams`.
/// The bytes those regions stand for sit **inside** the run's text, so they stay
/// there as data for the parse layer to re-allocate (extension mode: nothing is
/// attached, and the run's closer search already skips them — the construct reports
/// its own shape). A run of one physical line keeps the flush's allocation
/// (tasks #232/#295, byte-identical).
fn spans_lines(line: &[u8], run_start: usize, run_end: usize) -> bool {
    heredoc::line_start(line, run_start) < heredoc::line_start(line, run_end)
}
