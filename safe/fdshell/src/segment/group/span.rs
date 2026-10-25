//! The two byte scans that size a `{ …; }` group's span: the end of the
//! **statement** the group opens, and the opaque skip of a nested group.
//! Extracted from `segment/group.rs` for the line budget (task #315); the
//! branch that pushes the segment stays with the resume/floor rule.

use crate::brace::scan_brace_block;
use crate::scan::{Boundary, Quotes, ScanState, boundary, skip_comment};

use super::word::opens_at;

/// The end of the statement the group opens: the next separator (`;`, newline,
/// end of input) or `#` comment at or after `i`. A comment ends the statement
/// at its own byte and the scan resumes past the comment, so the comment's
/// words are never run and the next line stays outside the group's span.
/// A second `{ …; }` group in the same statement (after `&&` / `||`) is opaque:
/// its text is skipped whole, so the span ends past its closing brace and the
/// `cond` scan splits the statement into one part per group.
pub(crate) fn statement_end(line: &[u8], mut i: usize, quotes: Quotes) -> (usize, usize) {
    let mut state = quotes.scan_state();
    while line.get(i).is_some() {
        match boundary(line, i, &state) {
            Boundary::Separator => return (i, i + 1),
            Boundary::Comment => return (i, skip_comment(line, i)),
            // A group swallows a trailing `&` as word data (word_active is set
            // by the scan), exactly like a `Char`: the group's text carries the
            // `&` to the parse layer, which keeps its pinned group `&` error
            // (task #326 — group backgrounding stays #188/#192's).
            Boundary::Char | Boundary::Async => {
                i = if state.at_word_start() && opens_at(line, i) {
                    // The tail of a run-start group: its nested-group closedness
                    // is not this span's signal, so only the resume is taken.
                    skip_group(line, i, &mut state).0
                } else {
                    state.advance(line, i)
                };
            }
        }
    }
    (line.len(), line.len())
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
