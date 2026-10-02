//! Heredoc (`<<`) detection shared by the multi-line byte scanners (segment,
//! cond-list, comment, brace), so every call site agrees on which runs carry
//! heredoc bodies and where those bodies end. A body is opaque: it is skipped
//! whole, without folding its bytes into quote or substitution state.

mod body;
mod cont;
mod delims;
mod lines;
mod op;
mod regions;

use delims::operator_delims;

pub(crate) use body::body_spans;
pub(crate) use cont::unterminated;
pub(crate) use lines::first_unquoted_newline;
pub(crate) use op::{Operator, attached, invalid_delimiter, operator_delim};
pub(crate) use regions::body_regions;

/// If `line[run_start..run_end]` is a heredoc command line, return the index
/// of the newline terminating the last delimiter line (or `line.len()` when
/// that line is last). The body is opaque: the caller resumes just past the
/// returned newline. `None` when the run has no `<<` operator, when a bare
/// `<<` has no delimiter word, or when a delimiter line is missing.
pub(crate) fn skip(line: &[u8], run_start: usize, run_end: usize) -> Option<usize> {
    skip_region(line, run_start, run_end).map(|(_, resume, _)| resume)
}

/// `skip`, plus the opaque body span (first body byte, start of the last
/// delimiter line) and the statement span end: `Some((region, resume,
/// span_end))`. `span_end` is `resume`, except when the last delimiter is
/// empty — the blank line that terminates it is part of the statement, so
/// the span includes its terminating newline and the parser can see the
/// zero-length line.
pub(crate) fn skip_region(
    line: &[u8],
    run_start: usize,
    run_end: usize,
) -> Option<((usize, usize), usize, usize)> {
    let delims = operator_delims(line, run_start, run_end)?;
    if delims.is_empty() || !matches!(line.get(run_end), Some(&b'\n') | None) {
        return None;
    }
    let (spans, resume) = body_spans(line, run_end + 1, &delims).ok()?;
    // Non-empty by construction: `delims` is checked above and `spans` has
    // one entry per found delimiter.
    let first = spans.first()?.0;
    let last = spans.last()?.1;
    // An empty final delimiter extends the span through the blank line's
    // newline; the blank line always has one (a zero-byte final line never
    // matches), so `span_end` stays in bounds.
    let span_end = resume + usize::from(delims.last().is_some_and(|o| o.delim.is_empty()));
    Some(((first, last), resume, span_end))
}

#[cfg(test)]
mod tests;
