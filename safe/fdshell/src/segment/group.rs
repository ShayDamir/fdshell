use crate::brace::scan_brace_block;
use crate::scan::{Boundary, ScanState, boundary, skip_comment};
use crate::segment::book::Book;
use crate::segment::line;
use alloc::vec::Vec;

use super::Segment;

pub(super) mod opener;
mod prefix;
mod word;
pub(crate) use prefix::group_at_run;
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
    // flush claims only the line's heredoc bodies scanned so far.
    let pre_resume = line::flush_line(&mut book.segments, line, line_start, brace, &mut book.runs);
    let span_start = prefix::span_start(line, run_start, brace);
    let (segment, resume, floor) =
        group_block(line, brace, span_start, book.state.in_quote, pre_resume);
    book.segments.push(segment);
    book.rewind(resume, resume, floor);
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
/// the group on the same line (the line end when there is none).
fn group_block<'a>(
    line: &'a [u8],
    start: usize,
    block_start: usize,
    in_quote: bool,
    pre_resume: usize,
) -> (Segment<'a>, usize, usize) {
    let mut quote = in_quote;
    let (close, closed) = scan_brace_block(line, start + 1, &mut quote);
    // The group never closes: the span is the rest of the input, and the REPL
    // keeps reading continuation lines.
    let open_end = line.len().min(close + 1);
    let (end, resume, floor) = if closed {
        let (sep, r) = statement_end(line, close + 1, quote);
        (sep, r.max(pre_resume), r)
    } else {
        (open_end, open_end + 1, 0)
    };
    (
        Segment::Block {
            block_start,
            end_pos: end,
            closed,
            // A group's own heredocs are a separate limitation (task #226):
            // the group's span is carried verbatim, with no body regions.
            bodies: Vec::new(),
        },
        resume,
        floor,
    )
}

/// The end of the statement the group opens: the next separator (`;`, newline,
/// end of input) or `#` comment at or after `i`. A comment ends the statement
/// at its own byte and the scan resumes past the comment, so the comment's
/// words are never run and the next line stays outside the group's span.
/// A second `{ …; }` group in the same statement (after `&&` / `||`) is opaque:
/// its text is skipped whole, so the span ends past its closing brace and the
/// `cond` scan splits the statement into one part per group.
fn statement_end(line: &[u8], mut i: usize, in_quote: bool) -> (usize, usize) {
    let mut state = ScanState {
        in_quote,
        in_backtick: false,
        paren_depth: 0,
        word_active: false,
    };
    while line.get(i).is_some() {
        match boundary(line, i, &state) {
            Boundary::Separator => return (i, i + 1),
            Boundary::Comment => return (i, skip_comment(line, i)),
            Boundary::Char => {
                i = if state.at_word_start() && opens_at(line, i) {
                    skip_group(line, i, &mut state)
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
pub(super) fn skip_group(line: &[u8], i: usize, state: &mut ScanState) -> usize {
    let mut quote = state.in_quote;
    let (close, closed) = scan_brace_block(line, i + 1, &mut quote);
    state.in_quote = quote;
    state.word_active = false;
    if closed { close + 1 } else { line.len() }
}

#[cfg(test)]
mod tests;
