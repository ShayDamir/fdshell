//! Heredoc body-span resolution: where each delimiter line starts and where
//! the body ends.

use super::op::Operator;
use alloc::vec::Vec;

/// One operator's body span: `(body_start, body_end)` and whether its delimiter
/// line was found. The `found == false` shape is the **end-of-file** one (task
/// #364): no line matched because the text ended, so the span runs to the text
/// end and its body is whatever the stream leaves — bash reads it, warns, and
/// runs the command at rc 0, so the parse layer accepts it (its own consumers
/// keep the span's bytes opaque data).
pub(crate) struct Span {
    pub(crate) region: (usize, usize),
    pub(crate) found: bool,
}

/// The body spans of the delimiters in order plus the resume index (a found
/// delimiter line's terminating newline, or `line.len()` when its line is last
/// or never arrived). `from` is the body start, just after the command line's
/// newline. **One entry per operator**: the first operator whose delimiter line
/// is missing claims the rest of the stream and every operator behind it the
/// empty region at the text end (the allocation is one arithmetic unit with the
/// window, LESSONS).
pub(crate) fn body_spans(line: &[u8], from: usize, delims: &[Operator]) -> (Vec<Span>, usize) {
    let mut spans: Vec<Span> = Vec::new();
    let mut pos = from;
    let mut resume = from;
    for op in delims {
        let Some((start, end)) = next_delimiter(line, pos, op) else {
            spans.push(Span {
                region: (pos, line.len()),
                found: false,
            });
            resume = line.len();
            pos = line.len();
            continue;
        };
        spans.push(Span {
            region: (pos, start),
            found: true,
        });
        resume = end;
        pos = end + 1;
    }
    (spans, resume)
}

/// The next line matching `op` starting at `from`: the delimiter line's
/// start and the index of the newline terminating it (or `line.len()` when
/// the line is last). A zero-byte final line never matches, so an empty
/// delimiter needs a real blank line (a tabs-only line strips to empty and
/// does match a `<<-""` body).
pub(super) fn next_delimiter(line: &[u8], from: usize, op: &Operator) -> Option<(usize, usize)> {
    let mut i = from;
    while i < line.len() {
        let nl = line
            .get(i..)
            .and_then(|s| s.iter().position(|&b| b == b'\n'))
            .map(|p| i + p);
        let j = nl.unwrap_or(line.len());
        if op.matches(line.get(i..j)?) {
            return Some((i, j));
        }
        let n = nl?;
        i = n + 1;
    }
    None
}
