//! Heredoc (`<<`) detection shared by the multi-line byte scanners (segment,
//! cond-list, comment, brace), so every call site agrees on which runs carry
//! heredoc bodies and where those bodies end. A body is opaque: it is skipped
//! whole, without folding its bytes into quote or substitution state.

mod lines;
mod regions;

use super::ScanState;
use alloc::vec::Vec;

pub(crate) use lines::{body_spans, delimiter_word, first_unquoted_newline};
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
    let only: Vec<&[u8]> = delims.iter().map(|(d, _)| *d).collect();
    let (spans, resume) = body_spans(line, run_end + 1, &only).ok()?;
    // Non-empty by construction: `delims` is checked above and `spans` has
    // one entry per found delimiter.
    let first = spans.first()?.0;
    let last = spans.last()?.1;
    // An empty final delimiter extends the span through the blank line's
    // newline; the blank line always has one (a zero-byte final line never
    // matches), so `span_end` stays in bounds.
    let span_end = resume + usize::from(delims.last().is_some_and(|(d, _)| d.is_empty()));
    Some(((first, last), resume, span_end))
}

/// The `(delimiter, quoted)` pairs of every `<<` operator in the run, in
/// order. `None` when a bare `<<` has no delimiter word.
fn operator_delims(line: &[u8], from: usize, to: usize) -> Option<Vec<(&[u8], bool)>> {
    let mut state = ScanState::new();
    let mut found: Vec<(&[u8], bool)> = Vec::new();
    let mut seen_word = false;
    let mut i = from;
    while i < to {
        let bare = !state.in_quote && !state.in_backtick && state.dollar_paren_depth == 0;
        if bare
            && seen_word
            && word_start(line, i, &state)
            && line.get(i) == Some(&b'<')
            && line.get(i + 1) == Some(&b'<')
            && line.get(i + 2) != Some(&b'<')
            && !precedes_pipe(line, i)
        {
            let (next, raw) = lines::delimiter_span(line, i + 2, to)?;
            found.push(delimiter_word(raw));
            while i < next {
                i = state.advance(line, i);
            }
            seen_word = true;
            continue;
        }
        i = state.advance(line, i);
        if state.word_active {
            seen_word = true;
        }
    }
    Some(found)
}

/// `i` starts a token word: the run start or a tokenizer word-break byte.
/// A `)` breaks the word only at top level — a `$( )`-closing `)` keeps the
/// token whole, exactly as the tokenizer does.
fn word_start(line: &[u8], i: usize, state: &ScanState) -> bool {
    if i == 0 {
        return true;
    }
    match line.get(i - 1).copied() {
        Some(b')') => !state.word_active,
        _ => matches!(
            line.get(i - 1).copied(),
            Some(b' ') | Some(b'\t') | Some(b';') | Some(b'\n') | Some(b'|')
        ),
    }
}

/// A `|` with only whitespace between it and `i`: a pipeline position, where
/// `<<` is a command word, not an operator.
fn precedes_pipe(line: &[u8], i: usize) -> bool {
    let mut k = i;
    while k > 0 {
        k -= 1;
        match line.get(k) {
            Some(&b' ') | Some(b'\t') => continue,
            Some(b'|') => return true,
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests;
