//! The two byte scans that size a `{ …; }` group's span: the end of the
//! **statement** the group opens, and the opaque skip of a nested group.
//! Extracted from `segment/group.rs` for the line budget (task #315); the
//! branch that pushes the segment stays with the resume/floor rule.

use crate::brace::scan_brace_block;
use crate::scan::{Boundary, Quotes, ScanState, boundary, skip_comment};

use super::prefix::group_at_run;
use super::word::opens_at;

/// The end of the statement the group opens: the next separator (`;`, newline,
/// end of input) or `#` comment at or after `i`. A comment ends the statement
/// at its own byte and the scan resumes past the comment, so the comment's
/// words are never run and the next line stays outside the group's span.
/// A second `{ …; }` group in the same statement (after `&&` / `||`) is opaque:
/// its text is skipped whole, so the span ends past its closing brace and the
/// `cond` scan splits the statement into one part per group.
/// `run_start` is the span's start (the group's `{`, see `prefix::span_start`):
/// a tail brace must pass the run's command-position rule, exactly as
/// `cond/boundary.rs` gates `opens_at` — with end of input now a boundary
/// (task #402), an **argument** brace at the very end (`{ echo a; } && echo {`)
/// is word data and bash answers it at once, while a brace at a cond-part
/// start (`{ echo a; } && {`) is the opener.
/// The third element is the tail's closedness — `true` when every nested group
/// the scan skipped closed (task #401): the group's own `closed` measures its
/// own brace, so a tail left open rides on this flag to the REPL.
pub(crate) fn statement_end(
    line: &[u8],
    mut i: usize,
    quotes: Quotes,
    run_start: usize,
) -> (usize, usize, bool) {
    let mut state = quotes.scan_state();
    let mut tail_closed = true;
    while line.get(i).is_some() {
        match boundary(line, i, &state) {
            // A `;;` pair cuts the span exactly like the separator it is built
            // from (task #423): the span boundary is the same byte, and the
            // stray decision belongs to the parse layer's one predicate.
            Boundary::Separator | Boundary::SemiSemi => return (i, i + 1, tail_closed),
            Boundary::Comment => return (i, skip_comment(line, i), tail_closed),
            // A group swallows a trailing `&` as word data (word_active is set
            // by the scan), exactly like a `Char`: the group's text carries the
            // `&` to the parse layer, which keeps its pinned group `&` error
            // (task #326 — group backgrounding stays #188/#192's).
            Boundary::Char | Boundary::Async => {
                i = if state.at_word_start()
                    && opens_at(line, i)
                    && group_at_run(line, run_start, i).is_some()
                {
                    // The tail of a run-start group: its closedness is not this
                    // span's own `closed`, so it accumulates separately for the
                    // caller (task #401).
                    let (resume, closed) = skip_group(line, i, &mut state);
                    tail_closed &= closed;
                    resume
                } else {
                    state.advance(line, i)
                };
            }
        }
    }
    (line.len(), line.len(), tail_closed)
}

/// Skip the whole `{ …; }` group at `i` (a nested group in the same statement,
/// or a group inside a cond run): its text is opaque, so the braces and any
/// quote inside are counted, and the scan resumes just past its closing brace
/// (or at the end of input if unclosed). No `;` inside the body is a separator.
/// The pair is (resume, closed); an unclosed group is the cond run's mid-run
/// opener, so its caller records that for the REPL (task #294).
pub(crate) fn skip_group(line: &[u8], i: usize, state: &mut ScanState) -> (usize, bool) {
    let mut quotes = state.quotes();
    let (close, closed) = scan_brace_block(line, i + 1, &mut quotes);
    state.set_quotes(quotes);
    state.word_active = false;
    let resume = if closed { close + 1 } else { line.len() };
    (resume, closed)
}
