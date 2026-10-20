//! Split-time validation of the plain `&` async operator (task #326): the
//! scanner consumes the `&`, so the parse layer must never see a bare one,
//! and validating **at the split** is what keeps bash's preflight property —
//! `echo a & ; echo b` is a syntax error that runs **nothing**. An invalid
//! split appends `Segment::AsyncInvalid`, which `run_script` bails on before
//! executing any segment.

use super::Segment;
use super::book::Book;
use super::line::flush_line;

pub(super) mod split;
use split::Split;

/// Consume the async `&` at the cursor: mark the target run background, or
/// append the invalid-split marker when the split is invalid (the cursor
/// advances past the `&` either way). An invalid split also clears the
/// buffered runs: the pre-pass bails, so they must not leak into a later
/// statement's text.
pub(super) fn mark<'a>(line: &'a [u8], book: &mut Book<'a>) {
    let amp = book.i;
    let mut split = Split {
        line,
        start: book.start,
        i: book.i,
        pending: line.get(book.start..book.i).unwrap_or(b"").trim_ascii(),
        runs: &mut book.runs,
        segments: &mut book.segments,
        sep_since_run: book.sep_since_run,
    };
    if split.valid() {
        split.mark();
        // The `&` terminates the command list it marks: the buffered runs are
        // flushed now, so a later `&` can never target them again (bash's
        // `echo a & echo b &` is two background jobs, not one twice).
        let resume = flush_line(
            &mut book.segments,
            line,
            book.line_start,
            amp,
            &mut book.runs,
        )
        .max(book.body_floor);
        book.i = resume;
        book.start = book.i;
        book.line_start = book.line_start.max(amp);
        // The `&` terminates the run it marked: no `;` precedes it any more.
        book.sep_since_run = false;
    } else {
        book.segments.push(Segment::AsyncInvalid);
        book.runs.clear();
    }
    book.state.word_active = false;
    // Sole advancement rule: a flush that resumed at or before the `&` (an
    // empty target, a body-floor clamp) must never park the cursor on the
    // `&` it just consumed — the scan would re-read it forever.
    if book.i <= amp {
        book.start = amp + 1;
        book.i = amp + 1;
    }
}

/// A line whose first non-blank byte is `&` follows `i` (a newline split
/// defers its flush so the `&` can claim the runs buffered above it).
pub(super) fn async_ahead(line: &[u8], i: usize) -> bool {
    let mut j = i + 1;
    while matches!(line.get(j), Some(&b' ') | Some(&b'\t')) {
        j += 1;
    }
    line.get(j) == Some(&b'&')
}

#[cfg(test)]
mod tests;
