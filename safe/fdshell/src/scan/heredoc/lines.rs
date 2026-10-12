//! Word and line helpers for heredoc (`<<`) detection.

use super::super::ScanState;
use crate::bytes::{QUOTE, SQUOTE};

/// Strip one pair of surrounding quotes from the raw delimiter word and report
/// whether the word was quoted (`<<"Q"` → `Q`, quoted; `<<'Q'` → `Q`, quoted).
pub(crate) fn delimiter_word(raw: &[u8]) -> (&[u8], bool) {
    for q in [QUOTE, SQUOTE] {
        if raw.len() >= 2 && raw.first() == Some(&q) && raw.last() == Some(&q) {
            return (raw.get(1..raw.len() - 1).unwrap_or(b""), true);
        }
    }
    (raw, false)
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
        j = word_span(line, j, to);
    }
    if j > attached {
        return Some((j, line.get(attached..j)?));
    }
    while j < to && matches!(line.get(j).copied(), Some(b' ') | Some(b'\t')) {
        j += 1;
    }
    let word = j;
    while j < to && !token_word_end(line.get(j).copied().unwrap_or(0)) {
        j = word_span(line, j, to);
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

/// The index just past the delimiter word byte at `j`: a `'…'` span is
/// consumed whole, because the tokenizer reads it as one word and its spaces
/// are word content (`<<'a b'` delimits `a b`) — byte-level and token-level
/// rules must agree (LESSONS).
fn word_span(line: &[u8], j: usize, to: usize) -> usize {
    if line.get(j) != Some(&SQUOTE) {
        return j + 1;
    }
    let mut k = j + 1;
    while k < to && line.get(k) != Some(&SQUOTE) {
        k += 1;
    }
    k.saturating_add(1).min(to)
}

/// The index just after the first newline that is not inside quotes, a
/// single-quoted span, backticks, or a `$( )` substitution; `None` when the
/// line has none.
pub(crate) fn first_unquoted_newline(line: &[u8]) -> Option<usize> {
    let mut state = ScanState::new();
    let mut i = 0;
    while i < line.len() {
        let bare =
            !state.in_quote && !state.in_single && !state.in_backtick && state.paren_depth == 0;
        if bare && line.get(i) == Some(&b'\n') {
            return Some(i + 1);
        }
        i = state.advance(line, i);
    }
    None
}
