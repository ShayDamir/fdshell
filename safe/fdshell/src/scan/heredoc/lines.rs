//! Word and line helpers for heredoc (`<<`) detection.

use super::super::ScanState;
use alloc::vec::Vec;

/// Strip one pair of surrounding double quotes from the raw delimiter word
/// and report whether the word was quoted (`<<"Q"` → `Q`, quoted).
pub(crate) fn delimiter_word(raw: &[u8]) -> (&[u8], bool) {
    if raw.len() >= 2 && raw.first() == Some(&b'"') && raw.last() == Some(&b'"') {
        (raw.get(1..raw.len() - 1).unwrap_or(b""), true)
    } else {
        (raw, false)
    }
}

/// A byte that ends a token word for the tokenizer (the split set of
/// `token/step.rs`; `&`, `<`, `>` and `#` are ordinary word bytes).
pub(crate) fn token_word_end(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b';' | b'\n' | b'|' | b')')
}

/// The raw span of the delimiter word of the `<<` operator whose first byte
/// is two before `j`: the attached `<<WORD` span, or — for a bare `<<` — the
/// next word after whitespace. Returns the span end and raw bytes; `None`
/// when a bare `<<` has no delimiter word (the parser reports that).
pub(crate) fn delimiter_span(line: &[u8], mut j: usize, to: usize) -> Option<(usize, &[u8])> {
    let attached = j;
    while j < to && !token_word_end(line.get(j).copied().unwrap_or(0)) {
        j += 1;
    }
    if j > attached {
        return Some((j, line.get(attached..j)?));
    }
    while j < to && matches!(line.get(j).copied(), Some(b' ') | Some(b'\t')) {
        j += 1;
    }
    let word = j;
    while j < to && !token_word_end(line.get(j).copied().unwrap_or(0)) {
        j += 1;
    }
    // `word..j` is within `from..to` by construction.
    let raw = line.get(word..j)?;
    if raw.is_empty()
        || matches!(
            raw.first().copied(),
            Some(b'<') | Some(b'>') | Some(b'&') | Some(b'%')
        )
    {
        return None;
    }
    Some((j, raw))
}

/// The index just after the first newline that is not inside quotes,
/// backticks, or a `$( )` substitution; `None` when the line has none.
pub(crate) fn first_unquoted_newline(line: &[u8]) -> Option<usize> {
    let mut state = ScanState::new();
    let mut i = 0;
    while i < line.len() {
        let bare = !state.in_quote && !state.in_backtick && state.dollar_paren_depth == 0;
        if bare && line.get(i) == Some(&b'\n') {
            return Some(i + 1);
        }
        i = state.advance(line, i);
    }
    None
}

/// The `(body_start, delimiter_line_start)` spans of the delimiters in order
/// plus the resume index (the delimiter line's terminating newline, or
/// `line.len()` when the delimiter line is last). `from` is the body start,
/// just after the command line's newline. `Err(n)` when the (n+1)th
/// delimiter line is missing.
pub(crate) fn body_spans(
    line: &[u8],
    from: usize,
    delims: &[&[u8]],
) -> Result<(Vec<(usize, usize)>, usize), usize> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut pos = from;
    let mut resume = from;
    for (n, delim) in delims.iter().enumerate() {
        let Some((start, end)) = next_delimiter(line, pos, delim) else {
            return Err(n);
        };
        spans.push((pos, start));
        resume = end;
        pos = end + 1;
    }
    Ok((spans, resume))
}

/// The next line equal to `delim` starting at `from`: the delimiter line's
/// start and the index of the newline terminating it (or `line.len()` when
/// the line is last). A zero-byte final line never matches, so an empty
/// delimiter needs a real blank line.
fn next_delimiter(line: &[u8], from: usize, delim: &[u8]) -> Option<(usize, usize)> {
    let mut i = from;
    while i < line.len() {
        let nl = line
            .get(i..)
            .and_then(|s| s.iter().position(|&b| b == b'\n'))
            .map(|p| i + p);
        let j = nl.unwrap_or(line.len());
        if line.get(i..j) == Some(delim) {
            return Some((i, j));
        }
        let n = nl?;
        i = n + 1;
    }
    None
}
